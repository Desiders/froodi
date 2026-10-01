use super::{
    linking::{Link, LinkDependencies, Provider, RegistryIndex, SupportsExecution, SyncExecution},
    registry::RegistrationId,
    topology::Topology,
};
use crate::{
    utils::thread_safety::RcThreadSafety, Config, DependencyResolver, Finalizer, InstantiateErrorKind, Instantiator, Scope, ScopeData,
};
use alloc::vec::Vec;
use core::marker::PhantomData;

#[derive(Clone)]
pub enum NoFinalizer {}

impl<Out> Finalizer<Out> for NoFinalizer {
    fn finalize(&mut self, _: RcThreadSafety<Out>) {
        match *self {}
    }
}

pub struct Registration<Out, Inst, Deps, Fin> {
    pub(super) inst: Inst,
    pub(super) fin: Option<Fin>,
    pub(super) scope: ScopeData,
    pub(super) config: Config,
    pub(super) marker: PhantomData<fn() -> (Out, Deps)>,
}

pub fn reg<Inst: Provide<SyncExecution, Deps, Fin>, Deps, Fin>(
    scope: impl Scope,
    inst: Inst,
    config: Option<Config>,
    fin: Option<Fin>,
) -> Inst::Leaf {
    inst.into_leaf(scope.into(), config.unwrap_or_default(), fin)
}

pub trait Provide<Execution, Deps, Fin> {
    type Leaf;

    fn into_leaf(self, scope: ScopeData, config: Config, fin: Option<Fin>) -> Self::Leaf;
}

impl<Inst: Instantiator<Deps, Error = InstantiateErrorKind>, Deps: DependencyResolver, Fin> Provide<SyncExecution, Deps, Fin> for Inst {
    type Leaf = Registration<Inst::Provides, Inst, Deps, Fin>;

    fn into_leaf(self, scope: ScopeData, config: Config, fin: Option<Fin>) -> Self::Leaf {
        Registration {
            inst: self,
            fin,
            scope,
            config,
            marker: PhantomData,
        }
    }
}

unsafe impl<Out, Inst, Deps, Fin> RegistryIndex for Registration<Out, Inst, Deps, Fin> {
    type Index = Provider<Out>;
}

pub struct Linked<Out, Inst, Deps, Fin> {
    pub(super) reg: Registration<Out, Inst, Deps, Fin>,
    pub(super) targets: Vec<RegistrationId>,
}

unsafe impl<Root, Out, Inst, Deps, Fin, Links> Link<Root, Links> for Registration<Out, Inst, Deps, Fin>
where
    Deps: LinkDependencies<Root, Links>,
    Deps::Providers: SupportsExecution<SyncExecution>,
{
    type Linked = Linked<Out, Inst, Deps, Fin>;

    const TOPOLOGY: Topology = Topology::leaf(Deps::TARGETS);

    fn link(self) -> Self::Linked {
        Linked {
            reg: self,
            targets: Deps::TARGETS
                .iter()
                .filter_map(|target| target.map(|id| RegistrationId(u32::try_from(id).expect("too many registrations"))))
                .collect(),
        }
    }
}
