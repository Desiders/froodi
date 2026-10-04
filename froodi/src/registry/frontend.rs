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

#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct RegistrationId(pub(crate) u32);

impl RegistrationId {
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
    fn collect(self, entries: &mut Vec<CollectedRegistration>, runtime: &mut Vec<RuntimeRegistry>);
}

impl Collect for Empty {
    fn collect(self, _: &mut Vec<CollectedRegistration>, _: &mut Vec<RuntimeRegistry>) {}
}

impl<Left: Collect, Right: Collect> Collect for Node<Left, Right> {
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
    fn collect(self, entries: &mut Vec<CollectedRegistration>, _runtime: &mut Vec<RuntimeRegistry>) {
        let Registration {
            inst, fin, scope, config, ..
        } = self.reg;
        entries.push(CollectedRegistration {
            key: TypeInfo::of::<Out>(),
            data: Selected::Sync(InstantiatorData {
                instantiator: RegistrationInstantiator::linked::<Inst, Deps>(inst, self.targets),
                finalizer: fin.map(boxed_finalizer_factory),
                metadata: RegistrationMetadata {
                    dependencies: BTreeSet::new(),
                    scope_data: scope,
                    config,
                },
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
    fn collect(self, entries: &mut Vec<CollectedRegistration>, _runtime: &mut Vec<RuntimeRegistry>) {
        entries.push(CollectedRegistration {
            key: TypeInfo::of::<Container>(),
            data: Selected::Sync(InstantiatorData {
                instantiator: boxed_container_instantiator().into(),
                finalizer: None,
                metadata: RegistrationMetadata {
                    dependencies: BTreeSet::new(),
                    config: Config { cache_provides: false },
                    scope_data: self.scope,
                },
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
    type Index = crate::async_impl::typed_registration::AsyncProvider<crate::async_impl::Container>;
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
    fn collect(self, entries: &mut Vec<CollectedRegistration>, _: &mut Vec<RuntimeRegistry>) {
        entries.push(CollectedRegistration {
            key: TypeInfo::of::<crate::async_impl::Container>(),
            data: Selected::Async(crate::registry::AsyncInstantiatorData {
                instantiator: crate::async_impl::instantiator::boxed_container_instantiator().into(),
                finalizer: None,
                metadata: RegistrationMetadata {
                    dependencies: BTreeSet::new(),
                    config: Config { cache_provides: false },
                    scope_data: self.scope,
                },
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
        let mut entries = Vec::new();
        let mut runtime = Vec::new();
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
