use super::super::{
    boundary::{BoundaryLeaf, RuntimeNode},
    linking::{Empty, Link, Node, RegistryIndex},
    registration::{Linked, Registration},
    registry::{
        has_compiled_executors as has_compiled_sync_executors, prepare as prepare_sync, Collect, ContainerLeaf, IntoFragment, Registry,
    },
    topology::Topology,
    RegistrationId,
};
use super::registration::{AsyncLinked, AsyncProvider};
use crate::{
    async_impl::{
        finalizer::boxed_finalizer_factory,
        instantiator::{boxed_container_instantiator, RegistrationInstantiator as AsyncRegistrationInstantiator},
        registry::InstantiatorData as AsyncInstantiatorData,
        Container, Finalizer, Instantiator, Registry as AsyncRegistry, RegistryWithSync,
    },
    instantiator::RegistrationInstantiator,
    registry::InstantiatorData,
    utils::thread_safety::{SendSafety, SyncSafety},
    Config, Dependency, DependencyResolver, Registry as SyncRegistry, ScopeData, TypeInfo,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
};

#[derive(Clone)]
pub(crate) enum Selected {
    Sync(InstantiatorData),
    Async(AsyncInstantiatorData),
    Missing,
}

pub struct CollectedAsyncRegistration {
    pub(super) key: TypeInfo,
    pub(super) data: Selected,
}

pub trait CollectAsync {
    fn collect_async(self, entries: &mut Vec<CollectedAsyncRegistration>, runtime: &mut Vec<RegistryWithSync>);
}

impl CollectAsync for Empty {
    fn collect_async(self, _: &mut Vec<CollectedAsyncRegistration>, _: &mut Vec<RegistryWithSync>) {}
}

impl<Left: CollectAsync, Right: CollectAsync> CollectAsync for Node<Left, Right> {
    fn collect_async(self, entries: &mut Vec<CollectedAsyncRegistration>, runtime: &mut Vec<RegistryWithSync>) {
        self.0.collect_async(entries, runtime);
        self.1.collect_async(entries, runtime);
    }
}

impl<Out, Inst, Deps, Fin> CollectAsync for AsyncLinked<Out, Inst, Deps, Fin>
where
    Out: SendSafety + SyncSafety + 'static,
    Inst: Instantiator<Deps, Provides = Out> + SendSafety + SyncSafety,
    Deps: DependencyResolver + SendSafety + 'static,
    Fin: Finalizer<Out> + SendSafety + SyncSafety,
{
    fn collect_async(self, entries: &mut Vec<CollectedAsyncRegistration>, _: &mut Vec<RegistryWithSync>) {
        let Registration {
            inst, fin, scope, config, ..
        } = self.0.reg;
        let instantiator = AsyncRegistrationInstantiator::compiled::<Inst, Deps>(inst, self.0.targets);
        entries.push(CollectedAsyncRegistration {
            key: TypeInfo::of::<Out>(),
            data: Selected::Async(AsyncInstantiatorData {
                instantiator,
                dependencies: BTreeSet::new(),
                finalizer: fin.map(boxed_finalizer_factory),
                scope_data: scope,
                config,
            }),
        });
    }
}

macro_rules! sync_collect {
    ($ty:ty; $($param:ident),*) => {
        impl<$($param,)*> CollectAsync for $ty
        where
            Self: Collect,
        {
            fn collect_async(self, entries: &mut Vec<CollectedAsyncRegistration>, runtime: &mut Vec<RegistryWithSync>) {
                let mut sync = Vec::new();
                let mut fragments = Vec::new();
                self.collect(&mut sync, &mut fragments);
                entries.extend(sync.into_iter().map(|entry| CollectedAsyncRegistration {
                    key: entry.key,
                    data: entry.data.map_or(Selected::Missing, Selected::Sync),
                }));
                runtime.extend(fragments.into_iter().map(|sync| RegistryWithSync {
                    sync,
                    registry: AsyncRegistry::default(),
                }));
            }
        }
    };
}

sync_collect!(Linked<Out, Inst, Deps, Fin>; Out, Inst, Deps, Fin);
sync_collect!(ContainerLeaf;);
sync_collect!(RuntimeNode;);
sync_collect!(BoundaryLeaf<T>; T);

pub struct AsyncContainerLeaf {
    scope: ScopeData,
}

impl RegistryIndex for AsyncContainerLeaf {
    type Index = AsyncProvider<Container>;
}

impl<Root> Link<Root, ()> for AsyncContainerLeaf {
    type Linked = Self;

    const TOPOLOGY: Topology = Topology::leaf(&[]);

    fn link(self) -> Self {
        self
    }
}

impl CollectAsync for AsyncContainerLeaf {
    fn collect_async(self, entries: &mut Vec<CollectedAsyncRegistration>, _: &mut Vec<RegistryWithSync>) {
        entries.push(CollectedAsyncRegistration {
            key: TypeInfo::of::<Container>(),
            data: Selected::Async(AsyncInstantiatorData {
                instantiator: boxed_container_instantiator().into(),
                dependencies: BTreeSet::new(),
                finalizer: None,
                config: Config { cache_provides: false },
                scope_data: self.scope,
            }),
        });
    }
}

pub struct RuntimeAsyncNode(RegistryWithSync);

impl IntoFragment for RegistryWithSync {
    type Tree = RuntimeAsyncNode;

    fn into_fragment(self) -> (Self::Tree, Vec<ScopeData>) {
        let scopes = self.registry.scopes_data.clone();
        (RuntimeAsyncNode(self), scopes)
    }
}

impl IntoFragment for AsyncRegistry {
    type Tree = RuntimeAsyncNode;

    fn into_fragment(self) -> (Self::Tree, Vec<ScopeData>) {
        RegistryWithSync::from(self).into_fragment()
    }
}

impl RegistryIndex for RuntimeAsyncNode {
    type Index = Empty;
}

impl<Root> Link<Root, ()> for RuntimeAsyncNode {
    type Linked = Self;

    const TOPOLOGY: Topology = Topology::OPEN;

    fn link(self) -> Self {
        self
    }
}

impl CollectAsync for RuntimeAsyncNode {
    fn collect_async(self, _: &mut Vec<CollectedAsyncRegistration>, runtime: &mut Vec<RegistryWithSync>) {
        runtime.push(self.0);
    }
}

pub(super) type Root<Tree> = Node<Tree, Node<ContainerLeaf, AsyncContainerLeaf>>;
pub(super) type Index<Tree> = <Root<Tree> as RegistryIndex>::Index;

impl<Tree> Registry<Tree> {
    /// Materializes linked sync/async edges for later native registry composition.
    /// Cycle and scope validation are deferred to final container construction.
    pub fn into_async_registry<Links>(self) -> RegistryWithSync
    where
        Self: IntoRegistry<Links>,
    {
        IntoRegistry::into_registry(self)
    }
}

pub trait IntoRegistry<Links> {
    const VALIDATE: () = ();
    const CYCLES_CHECKED: bool = false;

    #[doc(hidden)]
    fn validate_runtime(_: &RegistryWithSync) {}

    fn into_registry(self) -> RegistryWithSync;

    #[doc(hidden)]
    fn materialize(self) -> (RegistryWithSync, Option<Vec<TypeInfo>>)
    where
        Self: Sized,
    {
        (self.into_registry(), None)
    }
}

impl IntoRegistry<()> for RegistryWithSync {
    fn into_registry(self) -> Self {
        self
    }
}

impl<Tree, Links> IntoRegistry<Links> for Registry<Tree>
where
    Root<Tree>: RegistryIndex + Link<Index<Tree>, Links>,
    <Root<Tree> as Link<Index<Tree>, Links>>::Linked: CollectAsync,
{
    const VALIDATE: () = <Root<Tree> as Link<Index<Tree>, Links>>::VALIDATE;
    const CYCLES_CHECKED: bool = <Root<Tree> as Link<Index<Tree>, Links>>::TOPOLOGY.can_validate_all_cycles();

    fn validate_runtime(registry: &RegistryWithSync) {
        if !has_compiled_sync_executors(&registry.sync) {
            if !Self::CYCLES_CHECKED {
                registry.sync.detect_cyclic_dependencies().expect("invalid compiled registry");
            }
            registry.sync.detect_unreachable_scopes().expect("invalid compiled registry");
        }
        if !has_compiled_executors(&registry.registry) {
            if !Self::CYCLES_CHECKED {
                registry
                    .registry
                    .detect_cyclic_dependencies()
                    .expect("invalid compiled async registry");
            }
            registry
                .registry
                .detect_unreachable_scopes()
                .expect("invalid compiled async registry");
        }
    }

    fn into_registry(self) -> RegistryWithSync {
        IntoRegistry::<Links>::materialize(self).0
    }

    fn materialize(self) -> (RegistryWithSync, Option<Vec<TypeInfo>>) {
        let mut scopes = self.scopes;
        scopes.sort_by_key(|scope| scope.priority);
        let root = Node(
            self.tree,
            Node(ContainerLeaf { scope: scopes[0] }, AsyncContainerLeaf { scope: scopes[0] }),
        );
        let mut entries = Vec::new();
        let mut runtime = Vec::new();
        root.link().collect_async(&mut entries, &mut runtime);
        assemble(
            entries,
            runtime,
            scopes,
            <Root<Tree> as Link<Index<Tree>, Links>>::TOPOLOGY.has_fixed_ids(),
        )
    }
}

// Keep runtime assembly closures independent of Root and Links.
fn assemble(
    entries: Vec<CollectedAsyncRegistration>,
    runtime: Vec<RegistryWithSync>,
    scopes: Vec<ScopeData>,
    fixed_ids: bool,
) -> (RegistryWithSync, Option<Vec<TypeInfo>>) {
    let keys: Vec<_> = entries.iter().map(|entry| entry.key.clone()).collect();
    let mut sync = SyncRegistry {
        scopes_data: scopes.clone(),
        ..SyncRegistry::default()
    };
    let mut registry = AsyncRegistry {
        scopes_data: scopes,
        ..AsyncRegistry::default()
    };
    for entry in entries {
        match entry.data {
            Selected::Sync(mut data) => {
                if let RegistrationInstantiator::Compiled(executor) = &mut data.instantiator {
                    executor.keys = executor.edges.iter().map(|id| keys[id.index()].clone()).collect::<Vec<_>>().into();
                    data.dependencies = executor.keys.iter().cloned().map(|type_info| Dependency { type_info }).collect();
                }
                sync.entries.insert(entry.key, data);
            }
            Selected::Async(mut data) => {
                if let AsyncRegistrationInstantiator::Compiled(executor) = &mut data.instantiator {
                    executor.keys = executor.edges.iter().map(|id| keys[id.index()].clone()).collect::<Vec<_>>().into();
                    data.dependencies = executor.keys.iter().cloned().map(|type_info| Dependency { type_info }).collect();
                }
                registry.entries.insert(entry.key, data);
            }
            Selected::Missing => (),
        }
    }
    for fragment in runtime {
        sync.entries.extend(fragment.sync.entries);
        registry.entries.extend(fragment.registry.entries);
    }
    (RegistryWithSync { registry, sync }, fixed_ids.then_some(keys))
}

pub(crate) fn prepare(mut registries: RegistryWithSync, cycles_checked: bool, plan: Option<Vec<TypeInfo>>) -> RegistryWithSync {
    let compiled_sync = has_compiled_sync_executors(&registries.sync);
    let sync_plan = if compiled_sync { plan.clone() } else { None };
    registries.sync = prepare_sync(registries.sync, cycles_checked, sync_plan);
    let registry = &mut registries.registry;
    let compiled_async = has_compiled_executors(registry);
    if !compiled_sync && !compiled_async {
        return registries;
    }
    if !cycles_checked {
        registry.detect_cyclic_dependencies().expect("invalid compiled async registry");
    }
    registry.detect_unreachable_scopes().expect("invalid compiled async registry");
    for (dependent, data) in &registry.entries {
        for dependency in &data.dependencies {
            let target_scope = registry
                .entries
                .get(&dependency.type_info)
                .map(|entry| entry.scope_data)
                .or_else(|| registries.sync.entries.get(&dependency.type_info).map(|entry| entry.scope_data));
            if let Some(dependency_scope) = target_scope {
                assert!(
                    data.scope_data.can_access(&dependency_scope),
                    "unreachable dependency {:?} from {:?}",
                    dependency.type_info,
                    dependent
                );
            }
        }
    }
    if !compiled_async {
        return registries;
    }
    let keys = if let Some(keys) = plan {
        keys
    } else {
        let mut keys: BTreeSet<_> = registry.entries.keys().chain(registries.sync.entries.keys()).cloned().collect();
        for entry in registry.entries.values() {
            if let AsyncRegistrationInstantiator::Compiled(executor) = &entry.instantiator {
                keys.extend(executor.keys.iter().cloned());
            }
        }
        let ids: BTreeMap<_, _> = keys
            .into_iter()
            .enumerate()
            .map(|(id, key)| (key, RegistrationId(u32::try_from(id).expect("too many registrations"))))
            .collect();
        for entry in registry.entries.values_mut() {
            if let AsyncRegistrationInstantiator::Compiled(executor) = &mut entry.instantiator {
                executor.remap(&ids);
            }
        }
        ids.into_keys().collect()
    };
    registry.indexed = keys
        .into_iter()
        .map(|key| {
            let data = registry
                .entries
                .get(&key)
                .cloned()
                .map(Selected::Async)
                .or_else(|| registries.sync.entries.get(&key).cloned().map(Selected::Sync))
                .unwrap_or(Selected::Missing);
            (key, data)
        })
        .collect();
    registries
}

pub(crate) fn finish<Links, Reg: IntoRegistry<Links>>(registry: Reg) -> RegistryWithSync {
    let () = Reg::VALIDATE;
    let (registry, plan) = registry.materialize();
    Reg::validate_runtime(&registry);
    // The same final topology covers both namespaces; erasure never retains this proof.
    prepare(registry, Reg::CYCLES_CHECKED, plan)
}

fn has_compiled_executors(registry: &AsyncRegistry) -> bool {
    registry
        .entries
        .values()
        .any(|entry| matches!(entry.instantiator, AsyncRegistrationInstantiator::Compiled(_)))
}

#[cfg(all(test, not(miri)))]
mod tests {
    extern crate std;

    use crate::{async_impl::Container, compiled_async_registry as async_registry, DefaultScope::*, Inject, InstantiateErrorKind};

    struct Value<const N: usize>(usize);

    async fn next<const N: usize, const PREV: usize>(Inject(previous): Inject<Value<PREV>>) -> Result<Value<N>, InstantiateErrorKind> {
        Ok(Value(previous.0 + 1))
    }

    #[tokio::test]
    async fn resolves_a_deep_async_chain_under_the_default_recursion_limit() {
        macro_rules! chain {
            ($($n:literal => $prev:literal),*) => {
                Container::new(async_registry! {
                    scope(App) [
                        provide(async || Ok::<_, InstantiateErrorKind>(Value::<0>(0))),
                        $(provide(next::<$n, $prev>),)*
                    ],
                })
            };
        }
        let container = chain!(
            1 => 0, 2 => 1, 3 => 2, 4 => 3, 5 => 4, 6 => 5, 7 => 6, 8 => 7, 9 => 8, 10 => 9,
            11 => 10, 12 => 11, 13 => 12, 14 => 13, 15 => 14, 16 => 15, 17 => 16, 18 => 17, 19 => 18, 20 => 19,
            21 => 20, 22 => 21, 23 => 22, 24 => 23, 25 => 24, 26 => 25, 27 => 26, 28 => 27, 29 => 28, 30 => 29,
            31 => 30, 32 => 31, 33 => 32, 34 => 33, 35 => 34, 36 => 35, 37 => 36, 38 => 37, 39 => 38, 40 => 39,
            41 => 40, 42 => 41, 43 => 42, 44 => 43, 45 => 44, 46 => 45, 47 => 46, 48 => 47, 49 => 48, 50 => 49,
            51 => 50, 52 => 51, 53 => 52, 54 => 53, 55 => 54, 56 => 55, 57 => 56, 58 => 57, 59 => 58, 60 => 59,
            61 => 60, 62 => 61, 63 => 62, 64 => 63, 65 => 64, 66 => 65, 67 => 66, 68 => 67, 69 => 68, 70 => 69,
            71 => 70, 72 => 71, 73 => 72, 74 => 73, 75 => 74, 76 => 75, 77 => 76, 78 => 77, 79 => 78, 80 => 79,
            81 => 80, 82 => 81, 83 => 82, 84 => 83, 85 => 84, 86 => 85, 87 => 86, 88 => 87, 89 => 88, 90 => 89,
            91 => 90, 92 => 91, 93 => 92, 94 => 93, 95 => 94, 96 => 95, 97 => 96, 98 => 97, 99 => 98
        );
        assert_eq!(container.get::<Value<99>>().await.unwrap().0, 99);
    }
}
