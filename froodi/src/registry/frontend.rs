use super::{
    linking::{Empty, Link, Node, Provider, RegistryIndex},
    registration::{Linked, LocatedRegistration, Registration},
    topology::Topology,
};
use crate::{
    finalizer::boxed_finalizer_factory,
    instantiator::{boxed_container_instantiator, RegistrationInstantiator},
    registry::{Entry, InstantiatorData, RegistrationMetadata, Selected},
    utils::thread_safety::{SendSafety, SyncSafety},
    Config, Container, DefaultScope, DependencyResolver, Finalizer, Instantiator, Registry as RuntimeRegistry, Scope, ScopeData, Scopes,
    TypeInfo,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec,
    vec::Vec,
};
use core::marker::PhantomData;

#[cfg(feature = "async")]
use crate::{
    async_impl::{
        instantiator::boxed_container_instantiator as boxed_async_container_instantiator, typed_registration::AsyncProvider,
        Container as AsyncContainer,
    },
    registry::AsyncInstantiatorData,
};

#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct RegistrationId(pub(crate) u32);

impl RegistrationId {
    #[cfg(all(
        feature = "std",
        feature = "thread_safe",
        not(feature = "lock-parking-lot"),
        not(feature = "lock-spin")
    ))]
    pub(crate) const UNINDEXED_LOCK: Self = Self(u32::MAX);

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

pub struct Registry<Tree> {
    pub(crate) tree: Tree,
    pub(crate) scopes: Vec<ScopeData>,
}

#[diagnostic::on_unimplemented(
    message = "a typed container requires concrete registrations in typed fragments",
    note = "use Container::new for native fragments or provider declarations"
)]
pub trait TypedRegistry {}

impl TypedRegistry for Empty {}

impl<Left: TypedRegistry, Right: TypedRegistry> TypedRegistry for Node<Left, Right> {}

impl<Out, Inst, Deps, Fin> TypedRegistry for Registration<Out, Inst, Deps, Fin> {}

impl<Reg: TypedRegistry, Source> TypedRegistry for LocatedRegistration<Reg, Source> {}

pub struct ScopeConverter<ScopeType, const N: usize>(PhantomData<fn() -> ScopeType>);

impl<ScopeType, const N: usize> ScopeConverter<ScopeType, N> {
    #[allow(clippy::unused_self)]
    pub fn convert<S: Scope + Scopes<N, Scope = ScopeType>>(&self, scope: S) -> ScopeData {
        scope.into()
    }
}

impl Registry<Empty> {
    pub fn empty() -> Self {
        let (_, scopes, _) = Self::scope_data(DefaultScope::Runtime);
        Self::from_parts(Empty, scopes)
    }

    #[doc(hidden)]
    pub fn scope_data<S: Scope + Scopes<N>, const N: usize>(scope: S) -> (ScopeData, Vec<ScopeData>, ScopeConverter<S::Scope, N>)
    where
        S::Scope: Into<ScopeData>,
    {
        let (root, children) = S::all();
        let mut scopes = vec![root.into()];
        scopes.extend(children.into_iter().map(Into::into));
        (scope.into(), scopes, ScopeConverter(PhantomData))
    }
}

impl<Tree> Registry<Tree> {
    pub fn from_parts(tree: Tree, scopes: Vec<ScopeData>) -> Self {
        Self { tree, scopes }
    }

    /// Materializes linked edges into a native registry for later composition.
    /// Cycle and scope validation are deferred to final container construction.
    pub fn into_registry<Links>(self) -> RuntimeRegistry
    where
        Self: IntoRegistry<Links>,
    {
        IntoRegistry::into_registry(self)
    }
}

pub trait IntoFragment {
    type Tree;

    fn into_fragment(self) -> (Self::Tree, Vec<ScopeData>);
}

impl<Tree> IntoFragment for Registry<Tree> {
    type Tree = Tree;

    fn into_fragment(self) -> (Tree, Vec<ScopeData>) {
        (self.tree, self.scopes)
    }
}

pub struct CollectedRegistration {
    pub(crate) key: TypeInfo,
    pub(crate) data: Selected,
}

pub trait Collect {
    // Reservations only: an uncounted implementation may still grow the vectors.
    const ENTRY_COUNT: usize = 0;
    const RUNTIME_COUNT: usize = 0;

    fn collect(self, entries: &mut Vec<CollectedRegistration>, runtime: &mut Vec<RuntimeRegistry>);
}

impl Collect for Empty {
    fn collect(self, _: &mut Vec<CollectedRegistration>, _: &mut Vec<RuntimeRegistry>) {}
}

impl<Left: Collect, Right: Collect> Collect for Node<Left, Right> {
    const ENTRY_COUNT: usize = Left::ENTRY_COUNT + Right::ENTRY_COUNT;
    const RUNTIME_COUNT: usize = Left::RUNTIME_COUNT + Right::RUNTIME_COUNT;

    fn collect(self, entries: &mut Vec<CollectedRegistration>, runtime: &mut Vec<RuntimeRegistry>) {
        self.0.collect(entries, runtime);
        self.1.collect(entries, runtime);
    }
}

impl<Out, Inst, Deps, Fin> Collect for Linked<Out, Inst, Deps, Fin>
where
    Out: SendSafety + SyncSafety + 'static,
    Inst: Instantiator<Deps, Provides = Out> + SendSafety + SyncSafety,
    Deps: DependencyResolver + 'static,
    Fin: Finalizer<Out> + SendSafety + SyncSafety,
{
    const ENTRY_COUNT: usize = 1;

    fn collect(self, entries: &mut Vec<CollectedRegistration>, _runtime: &mut Vec<RuntimeRegistry>) {
        let Registration {
            inst, fin, scope, config, ..
        } = self.reg;
        entries.push(CollectedRegistration {
            key: TypeInfo::of::<Out>(),
            data: Selected::Sync(InstantiatorData {
                instantiator: RegistrationInstantiator::linked::<Inst, Deps>(inst, self.targets),
                finalizer: fin.map(boxed_finalizer_factory),
                metadata: RegistrationMetadata::new(BTreeSet::new(), config, scope),
            }),
        });
    }
}

pub struct ContainerLeaf {
    pub(crate) scope: ScopeData,
}

impl RegistryIndex for ContainerLeaf {
    type Index = Provider<Container>;
}

impl<Root> Link<Root, ()> for ContainerLeaf {
    type Linked = Self;

    const TOPOLOGY: Topology = Topology::leaf(&[]);

    fn link(self) -> Self {
        self
    }
}

impl Collect for ContainerLeaf {
    const ENTRY_COUNT: usize = 1;

    fn collect(self, entries: &mut Vec<CollectedRegistration>, _runtime: &mut Vec<RuntimeRegistry>) {
        entries.push(CollectedRegistration {
            key: TypeInfo::of::<Container>(),
            data: Selected::Sync(InstantiatorData {
                instantiator: boxed_container_instantiator().into(),
                finalizer: None,
                metadata: RegistrationMetadata::new(BTreeSet::new(), Config { cache_provides: false }, self.scope),
            }),
        });
    }
}

#[cfg(not(feature = "async"))]
pub(crate) type Root<Tree> = Node<Tree, ContainerLeaf>;
#[cfg(feature = "async")]
pub(crate) type Root<Tree> = Node<Tree, Node<ContainerLeaf, AsyncContainerLeaf>>;
#[cfg(feature = "async")]
pub struct AsyncContainerLeaf {
    scope: ScopeData,
}

#[cfg(feature = "async")]
impl RegistryIndex for AsyncContainerLeaf {
    type Index = AsyncProvider<AsyncContainer>;
}

#[cfg(feature = "async")]
impl<Root> Link<Root, ()> for AsyncContainerLeaf {
    type Linked = Self;

    const TOPOLOGY: Topology = Topology::leaf(&[]);

    fn link(self) -> Self {
        self
    }
}

#[cfg(feature = "async")]
impl Collect for AsyncContainerLeaf {
    const ENTRY_COUNT: usize = 1;

    fn collect(self, entries: &mut Vec<CollectedRegistration>, _: &mut Vec<RuntimeRegistry>) {
        entries.push(CollectedRegistration {
            key: TypeInfo::of::<AsyncContainer>(),
            data: Selected::Async(AsyncInstantiatorData {
                instantiator: boxed_async_container_instantiator().into(),
                finalizer: None,
                metadata: RegistrationMetadata::new(BTreeSet::new(), Config { cache_provides: false }, self.scope),
            }),
        });
    }
}

pub(crate) type Index<Tree> = <Root<Tree> as RegistryIndex>::Index;

pub trait IntoRegistry<Links> {
    const VALIDATE: () = ();
    const CYCLES_CHECKED: bool = false;

    fn into_registry(self) -> RuntimeRegistry;

    #[doc(hidden)]
    fn materialize(self) -> (RuntimeRegistry, Option<Vec<TypeInfo>>)
    where
        Self: Sized,
    {
        (self.into_registry(), None)
    }
}

impl IntoRegistry<()> for RuntimeRegistry {
    fn into_registry(self) -> Self {
        self
    }
}

impl<Tree, Links> IntoRegistry<Links> for Registry<Tree>
where
    Root<Tree>: RegistryIndex + Link<Index<Tree>, Links>,
    <Root<Tree> as Link<Index<Tree>, Links>>::Linked: Collect,
{
    const VALIDATE: () = <Root<Tree> as Link<Index<Tree>, Links>>::VALIDATE;
    const CYCLES_CHECKED: bool = <Root<Tree> as Link<Index<Tree>, Links>>::TOPOLOGY.can_validate_all_cycles();

    fn into_registry(self) -> RuntimeRegistry {
        IntoRegistry::<Links>::materialize(self).0
    }

    fn materialize(self) -> (RuntimeRegistry, Option<Vec<TypeInfo>>) {
        let mut scopes = self.scopes;
        scopes.sort_by_key(|scope| scope.priority);
        #[cfg(not(feature = "async"))]
        let root = Node(self.tree, ContainerLeaf { scope: scopes[0] });
        #[cfg(feature = "async")]
        let root = Node(
            self.tree,
            Node(ContainerLeaf { scope: scopes[0] }, AsyncContainerLeaf { scope: scopes[0] }),
        );
        let mut entries = Vec::with_capacity(<<Root<Tree> as Link<Index<Tree>, Links>>::Linked as Collect>::ENTRY_COUNT);
        let mut runtime = Vec::with_capacity(<<Root<Tree> as Link<Index<Tree>, Links>>::Linked as Collect>::RUNTIME_COUNT);
        root.link().collect(&mut entries, &mut runtime);
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
    entries: Vec<CollectedRegistration>,
    runtime: Vec<RuntimeRegistry>,
    scopes: Vec<ScopeData>,
    fixed_ids: bool,
) -> (RuntimeRegistry, Option<Vec<TypeInfo>>) {
    let keys: Vec<_> = entries.iter().map(|entry| entry.key.clone()).collect();
    let mut registry = RuntimeRegistry {
        scopes_data: scopes,
        ..RuntimeRegistry::default()
    };
    for mut entry in entries {
        entry.data.link_keys(&keys);
        if !matches!(entry.data, Selected::Missing) {
            registry.insert(entry.key, entry.data);
        }
    }
    for fragment in runtime {
        registry.extend(fragment);
    }
    (registry, fixed_ids.then_some(keys))
}

pub(crate) fn prepare(mut registry: RuntimeRegistry, cycles_checked: bool, plan: Option<Vec<TypeInfo>>) -> RuntimeRegistry {
    if !cycles_checked {
        registry.detect_cyclic_dependencies().expect("invalid registry");
    }
    registry
        .detect_unreachable_scopes()
        .expect("invalid registry: unreachable dependency or incompatible execution kind");
    if !has_linked_instantiators(&registry) {
        return registry;
    }
    // Only final typed composition retains declaration-order IDs. Erasure discards this plan.
    let keys = if let Some(keys) = plan {
        keys
    } else {
        let mut keys: BTreeSet<_> = registry.entries.keys().cloned().collect();
        for entry in registry.entries.values() {
            keys.extend(entry.linked_keys().iter().cloned());
            keys.extend(entry.fallback_keys().iter().cloned());
        }
        let ids: BTreeMap<_, _> = keys
            .into_iter()
            .enumerate()
            .map(|(id, key)| (key, RegistrationId(u32::try_from(id).expect("too many registrations"))))
            .collect();
        for entry in registry.entries.values_mut() {
            entry.remap(&ids);
        }
        ids.into_keys().collect()
    };
    // Lock IDs occupy a separate index domain from graph edges: one per final TypeId.
    // Entries are unique by TypeId; sync fallback receives the same ID before cloning.
    #[cfg(all(
        feature = "std",
        feature = "thread_safe",
        not(feature = "lock-parking-lot"),
        not(feature = "lock-spin")
    ))]
    for (index, entry) in registry.entries.values_mut().enumerate() {
        entry.set_lock_id(RegistrationId(u32::try_from(index).expect("too many registrations")));
    }
    registry.indexed = keys
        .into_iter()
        .map(|key| {
            let data = registry
                .entries
                .get(&key)
                .cloned()
                .unwrap_or_else(|| Entry::from_selected(Selected::Missing));
            (key, data)
        })
        .collect();
    registry
}

pub(crate) fn finish<Links, Reg: IntoRegistry<Links>>(registry: Reg) -> RuntimeRegistry {
    let () = Reg::VALIDATE;
    let (registry, plan) = registry.materialize();
    // Consume the proof only for this final composition; native registries retain no exemption.
    prepare(registry, Reg::CYCLES_CHECKED, plan)
}

pub(crate) fn has_linked_instantiators(registry: &RuntimeRegistry) -> bool {
    registry.entries.values().any(Entry::is_linked)
}

#[cfg(all(
    test,
    feature = "std",
    feature = "thread_safe",
    not(feature = "lock-parking-lot"),
    not(feature = "lock-spin")
))]
mod once_lock_metadata_tests {
    #[cfg(feature = "async")]
    use super::AsyncContainer;
    use super::{prepare, RegistrationId, RuntimeRegistry};
    use crate::{lock::PerTypeSyncLocks, registry, Container, DefaultScope::App, TypeInfo};
    use alloc::vec;
    #[cfg(feature = "async")]
    use alloc::vec::Vec;
    use core::ptr::eq as ptr_eq;

    #[test]
    fn duplicate_graph_ids_and_type_lookup_share_a_unique_type_lock() {
        let registry = registry! { provide(App, || Ok(7_u32)) }.into_registry();
        #[allow(unused_mut)]
        let mut keys = vec![TypeInfo::of::<u32>(), TypeInfo::of::<u32>(), TypeInfo::of::<Container>()];
        #[cfg(feature = "async")]
        keys.push(TypeInfo::of::<AsyncContainer>());
        let registry = prepare(registry, false, Some(keys));
        let by_type = registry.get(&TypeInfo::of::<u32>()).unwrap().metadata.lock_id;
        let first_graph = registry.indexed[0].1.sync().unwrap().metadata.lock_id;
        let duplicate_graph = registry.indexed[1].1.sync().unwrap().metadata.lock_id;
        let canonical = registry.entries.keys().position(|key| key.id == TypeInfo::of::<u32>().id).unwrap();
        assert_eq!(by_type.index(), canonical);
        assert_eq!(first_graph.index(), canonical);
        assert_eq!(duplicate_graph.index(), canonical);
        let locks = PerTypeSyncLocks::with_indexed_keys(registry.dense_lock_keys());
        let ordinary = locks.get(TypeInfo::of::<u32>().id, by_type);
        let linked = locks.get(TypeInfo::of::<u32>().id, duplicate_graph);
        assert!(ptr_eq(&*ordinary, &*linked));
    }

    #[test]
    fn descendants_share_the_same_root_mutex_and_legacy_stays_unindexed() {
        let app = Container::new(registry! { provide(App, || Ok(7_u32)) });
        let request = app.clone().enter_build().unwrap();
        let id = app.inner.registry.get(&TypeInfo::of::<u32>()).unwrap().metadata.lock_id;
        let parent = app.per_type_locks.get(TypeInfo::of::<u32>().id, id);
        let child = request.per_type_locks.get(TypeInfo::of::<u32>().id, id);
        assert!(ptr_eq(&*parent, &*child));
        let legacy = Container::new(RuntimeRegistry::new_with_default_entries());
        assert!(legacy.inner.registry.indexed.is_empty());
        assert_eq!(legacy.inner.registry.dense_lock_keys().count(), 0);
        let implicit = legacy.get::<Container>().unwrap();
        let id = RegistrationId::UNINDEXED_LOCK;
        let outer = legacy.per_type_locks.get(TypeInfo::of::<Container>().id, id);
        let inner = implicit.per_type_locks.get(TypeInfo::of::<Container>().id, id);
        assert!(ptr_eq(&*outer, &*inner));
    }

    #[cfg(feature = "async")]
    #[test]
    fn erased_mixed_sync_fallback_matches_async_and_indexed_metadata() {
        let synchronous = registry! { provide(App, || Ok(7_u32)) }.into_registry();
        let asynchronous = registry! { provide(App, async || Ok(9_u32)) }.into_registry();
        let registry = prepare(registry! { extend(synchronous, asynchronous) }.into_registry(), false, None);
        let sync_id = registry.get(&TypeInfo::of::<u32>()).unwrap().metadata.lock_id.index();
        let async_id = registry.get_async(&TypeInfo::of::<u32>()).unwrap().metadata.lock_id.index();
        assert_eq!(sync_id, async_id);
        let dense_keys: Vec<_> = registry.dense_lock_keys().collect();
        assert_eq!(dense_keys[sync_id], TypeInfo::of::<u32>().id);
        let indexed = registry.indexed.iter().find(|(key, _)| key.id == TypeInfo::of::<u32>().id).unwrap();
        assert_eq!(indexed.1.sync().unwrap().metadata.lock_id.index(), sync_id);
    }
}
