use super::{
    linking::{Link, LinkDependencies, Provider, RegistryIndex, SupportsExecution, SyncExecution},
    registry::RegistrationId,
    topology::Topology,
};
use crate::{utils::thread_safety::RcThreadSafety, Config, DependencyResolver, Finalizer, InstantiateErrorKind, Instantiator, ScopeData};
use alloc::vec::Vec;
use core::marker::PhantomData;

// Macro-generated labels survive only until linking, never in the runtime executor.
pub trait RegistrationSource {
    const DESCRIPTION: &'static str;
}

pub struct LocatedRegistration<Reg, Source> {
    reg: Reg,
    marker: PhantomData<fn() -> Source>,
}

impl<Reg, Source> LocatedRegistration<Reg, Source> {
    pub fn new(reg: Reg) -> Self {
        Self { reg, marker: PhantomData }
    }
}

impl<Reg: RegistryIndex, Source> RegistryIndex for LocatedRegistration<Reg, Source> {
    type Index = Reg::Index;
}

impl<Root, Links, Reg: Link<Root, Links>, Source: RegistrationSource> Link<Root, Links> for LocatedRegistration<Reg, Source> {
    type Linked = Reg::Linked;

    const TOPOLOGY: Topology = Reg::TOPOLOGY.with_source(Source::DESCRIPTION);

    fn link(self) -> Self::Linked {
        self.reg.link()
    }
}

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
    scope: ScopeData,
    inst: Inst,
    config: Option<Config>,
    fin: Option<Fin>,
) -> Inst::Leaf {
    inst.into_leaf(scope, config.unwrap_or_default(), fin)
}

#[diagnostic::on_unimplemented(
    message = "`provide(...)` requires a Froodi instantiator; `{Self}` is not supported",
    label = "expected an instantiator returning `Result<T, InstantiateErrorKind>` (or a future returning it for async registration)",
    note = "instantiators must be Clone + 'static, with resolvable parameters and Error = InstantiateErrorKind"
)]
pub trait Provide<Execution, Deps, Fin> {
    type Leaf;

    fn into_leaf(self, scope: ScopeData, config: Config, fin: Option<Fin>) -> Self::Leaf;
}

#[diagnostic::do_not_recommend]
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

impl<Out, Inst, Deps, Fin> RegistryIndex for Registration<Out, Inst, Deps, Fin> {
    type Index = Provider<Out>;
}

pub struct Linked<Out, Inst, Deps, Fin> {
    pub(super) reg: Registration<Out, Inst, Deps, Fin>,
    pub(super) targets: Vec<RegistrationId>,
}

impl<Root, Out, Inst, Deps, Fin, Links> Link<Root, Links> for Registration<Out, Inst, Deps, Fin>
where
    Deps: LinkDependencies<Root, Links>,
    Deps::Providers: SupportsExecution<SyncExecution, Deps>,
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
