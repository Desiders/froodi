use super::{
    frontend::RegistrationId,
    linking::{Link, LinkDependencies, Provider, RegistryIndex, SupportsExecution, SyncExecution},
    static_scope::{DynamicScope, ScopeType, StaticRegistrationSource, TypedScopeData},
    topology::Topology,
};
use crate::{
    utils::thread_safety::RcThreadSafety, Config, DependencyResolver, Finalizer, InstantiateErrorKind, Instantiator, ScopeData, StaticScope,
};
use alloc::vec::Vec;
use core::marker::PhantomData;

pub trait RegistrationSource {
    const DESCRIPTION: &'static str;
    const SCOPE: Option<&'static ScopeData> = None;
}

pub struct LocatedRegistration<Reg, Source> {
    reg: Reg,
    marker: PhantomData<fn() -> Source>,
}

#[allow(clippy::unused_self)]
impl TypedScopeData<DynamicScope> {
    pub fn locate<Reg, Source>(self, reg: Reg) -> LocatedRegistration<Reg, Source> {
        LocatedRegistration { reg, marker: PhantomData }
    }
}

#[allow(clippy::unused_self)]
impl<S: StaticScope> TypedScopeData<ScopeType<S>> {
    pub fn locate<Reg, Source>(self, reg: Reg) -> LocatedRegistration<Reg, StaticRegistrationSource<Source, S>> {
        LocatedRegistration { reg, marker: PhantomData }
    }
}

impl<Reg: RegistryIndex, Source> RegistryIndex for LocatedRegistration<Reg, Source> {
    type Index = Reg::Index;
}

impl<Root, Links, Reg: Link<Root, Links>, Source: RegistrationSource> Link<Root, Links> for LocatedRegistration<Reg, Source> {
    type Linked = Reg::Linked;

    const TOPOLOGY: Topology = Reg::TOPOLOGY.with_source(Source::DESCRIPTION).with_scope(Source::SCOPE);

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
    pub(crate) inst: Inst,
    pub(crate) fin: Option<Fin>,
    pub(crate) scope: ScopeData,
    pub(crate) config: Config,
    pub(crate) marker: PhantomData<fn() -> (Out, Deps)>,
}

pub fn reg<Inst: Provide<Execution, Deps, Fin>, Execution, Deps, Fin>(
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
    pub(crate) reg: Registration<Out, Inst, Deps, Fin>,
    pub(crate) targets: Vec<RegistrationId>,
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
