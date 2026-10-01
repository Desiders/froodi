use super::{
    linking::{Empty, Link, Node, Provider, RegistryIndex},
    registration::{Linked, Registration},
    topology::Topology,
};
use crate::{
    finalizer::boxed_finalizer_factory,
    instantiator::{boxed_container_instantiator, RegistrationInstantiator},
    registry::InstantiatorData,
    utils::thread_safety::{SendSafety, SyncSafety},
    Config, Container, DefaultScope, Dependency, DependencyResolver, Finalizer, Instantiator, Registry as RuntimeRegistry, Scope,
    ScopeData, Scopes, TypeInfo,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec,
    vec::Vec,
};

#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct RegistrationId(pub(crate) u32);

impl RegistrationId {
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

pub struct Registry<Tree> {
    pub(super) tree: Tree,
    pub(super) scopes: Vec<ScopeData>,
}

impl Registry<Empty> {
    pub fn empty() -> Self {
        Self::from_tree::<DefaultScope, 5, 0>(Empty, [])
    }
}

impl<Tree> Registry<Tree> {
    pub fn from_tree<S: Scope + Scopes<N, Scope = S>, const N: usize, const M: usize>(tree: Tree, _: [S; M]) -> Self {
        let (root, children) = S::all();
        let mut scopes = vec![root.into()];
        scopes.extend(children.into_iter().map(Into::into));
        Self { tree, scopes }
    }

    pub fn from_parts(tree: Tree, scopes: Vec<ScopeData>) -> Self {
        Self { tree, scopes }
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
    pub(super) key: TypeInfo,
    pub(super) data: Option<InstantiatorData>,
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
            data: Some(InstantiatorData {
                // SAFETY: linking preserved parameter order and target types; assembly remaps IDs before execution.
                instantiator: unsafe { RegistrationInstantiator::compiled::<Inst, Deps>(inst, self.targets) },
                dependencies: BTreeSet::new(),
                finalizer: fin.map(boxed_finalizer_factory),
                scope_data: scope,
                config,
            }),
        });
    }
}

pub struct ContainerLeaf {
    pub(super) scope: ScopeData,
}

unsafe impl RegistryIndex for ContainerLeaf {
    type Index = Provider<Container>;
}

unsafe impl<Root> Link<Root, ()> for ContainerLeaf {
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
            data: Some(InstantiatorData {
                instantiator: boxed_container_instantiator().into(),
                dependencies: BTreeSet::new(),
                finalizer: None,
                config: Config { cache_provides: false },
                scope_data: self.scope,
            }),
        });
    }
}

type Root<Tree> = Node<Tree, ContainerLeaf>;
type Index<Tree> = <Root<Tree> as RegistryIndex>::Index;

pub trait IntoRegistry<Links> {
    const VALIDATE: () = ();

    #[doc(hidden)]
    fn validate_runtime(_: &RuntimeRegistry) {}

    fn into_registry(self) -> RuntimeRegistry;
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

    fn validate_runtime(registry: &RuntimeRegistry) {
        if !has_compiled_executors(registry) {
            registry.validate().expect("invalid compiled registry");
        }
    }

    fn into_registry(self) -> RuntimeRegistry {
        let mut scopes = self.scopes;
        scopes.sort_by_key(|scope| scope.priority);
        let root = Node(self.tree, ContainerLeaf { scope: scopes[0] });
        let mut entries = Vec::new();
        let mut runtime = Vec::new();
        root.link().collect(&mut entries, &mut runtime);
        assemble(entries, runtime, scopes)
    }
}

// Keep runtime assembly closures independent of Root and Links.
fn assemble(mut entries: Vec<CollectedRegistration>, runtime: Vec<RuntimeRegistry>, scopes: Vec<ScopeData>) -> RuntimeRegistry {
    let keys: Vec<_> = entries.iter().map(|entry| entry.key.clone()).collect();
    for entry in &mut entries {
        let Some(data) = &mut entry.data else { continue };
        if let RegistrationInstantiator::Compiled(executor) = &mut data.instantiator {
            executor.keys = executor.edges.iter().map(|id| keys[id.index()].clone()).collect::<Vec<_>>().into();
            data.dependencies = executor.keys.iter().cloned().map(|type_info| Dependency { type_info }).collect();
        }
    }
    let mut registry = RuntimeRegistry {
        entries: entries
            .into_iter()
            .filter_map(|entry| entry.data.map(|data| (entry.key, data)))
            .collect(),
        scopes_data: scopes,
        indexed: Vec::new(),
    };
    for fragment in runtime {
        registry.entries.extend(fragment.entries);
    }
    registry
}

pub(crate) fn prepare(mut registry: RuntimeRegistry) -> RuntimeRegistry {
    if !has_compiled_executors(&registry) {
        return registry;
    }
    registry.validate().expect("invalid compiled registry");
    let mut keys: BTreeSet<_> = registry.entries.keys().cloned().collect();
    for entry in registry.entries.values() {
        if let RegistrationInstantiator::Compiled(executor) = &entry.instantiator {
            keys.extend(executor.keys.iter().cloned());
        }
    }
    let ids: BTreeMap<_, _> = keys
        .into_iter()
        .enumerate()
        .map(|(id, key)| (key, RegistrationId(u32::try_from(id).expect("too many registrations"))))
        .collect();
    for entry in registry.entries.values_mut() {
        if let RegistrationInstantiator::Compiled(executor) = &mut entry.instantiator {
            executor.remap(&ids);
        }
    }
    registry.indexed = ids.keys().map(|key| (key.clone(), registry.entries.get(key).cloned())).collect();
    registry
}

pub(crate) fn finish<Links, Reg: IntoRegistry<Links>>(registry: Reg) -> RuntimeRegistry {
    let () = Reg::VALIDATE;
    let registry = registry.into_registry();
    Reg::validate_runtime(&registry);
    registry
}

pub(super) fn has_compiled_executors(registry: &RuntimeRegistry) -> bool {
    registry
        .entries
        .values()
        .any(|entry| matches!(entry.instantiator, RegistrationInstantiator::Compiled(_)))
}
