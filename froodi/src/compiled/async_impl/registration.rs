use super::super::{
    linking::{AsyncExecution, Here, Link, LinkDependencies, ProviderPath, RegistryIndex, Size, SupportsExecution},
    registration::{Linked, NoFinalizer, Provide, Registration},
    topology::Topology,
    RegistrationId,
};
use crate::{
    async_impl::{Finalizer, Instantiator},
    utils::thread_safety::{RcThreadSafety, SendSafety},
    Config, DependencyResolver, InstantiateErrorKind, ScopeData,
};
use core::{
    future::{ready, Future},
    marker::PhantomData,
};

pub struct AsyncRegistration<Out, Inst, Deps, Fin>(pub(super) Registration<Out, Inst, Deps, Fin>);

pub fn async_reg<Inst: Provide<AsyncExecution, Deps, Fin>, Deps, Fin>(
    scope: ScopeData,
    inst: Inst,
    config: Option<Config>,
    fin: Option<Fin>,
) -> Inst::Leaf {
    inst.into_leaf(scope, config.unwrap_or_default(), fin)
}

#[diagnostic::do_not_recommend]
impl<Inst: Instantiator<Deps, Error = InstantiateErrorKind>, Deps: DependencyResolver, Fin> Provide<AsyncExecution, Deps, Fin> for Inst {
    type Leaf = AsyncRegistration<Inst::Provides, Inst, Deps, Fin>;

    fn into_leaf(self, scope: ScopeData, config: Config, fin: Option<Fin>) -> Self::Leaf {
        AsyncRegistration(Registration {
            inst: self,
            fin,
            scope,
            config,
            marker: PhantomData,
        })
    }
}

pub struct AsyncProvider<Out>(PhantomData<fn() -> Out>);

impl<Out> Size for AsyncProvider<Out> {
    const SIZE: usize = 1;
}

impl<Out> ProviderPath<Out, Here> for AsyncProvider<Out> {
    type Provider = Self;

    const INDEX: usize = 0;
}

impl<Out, Request> SupportsExecution<AsyncExecution, Request> for AsyncProvider<Out> {}

impl<Out, Inst, Deps, Fin> RegistryIndex for AsyncRegistration<Out, Inst, Deps, Fin> {
    type Index = AsyncProvider<Out>;
}

pub struct AsyncLinked<Out, Inst, Deps, Fin>(pub(super) Linked<Out, Inst, Deps, Fin>);

impl<Root, Out, Inst, Deps, Fin, Links> Link<Root, Links> for AsyncRegistration<Out, Inst, Deps, Fin>
where
    Deps: LinkDependencies<Root, Links>,
    Deps::Providers: SupportsExecution<AsyncExecution, Deps>,
{
    type Linked = AsyncLinked<Out, Inst, Deps, Fin>;

    const TOPOLOGY: Topology = Topology::leaf(Deps::TARGETS);

    fn link(self) -> Self::Linked {
        AsyncLinked(Linked {
            reg: self.0,
            targets: Deps::TARGETS
                .iter()
                .filter_map(|target| target.map(|id| RegistrationId(u32::try_from(id).expect("too many registrations"))))
                .collect(),
        })
    }
}

impl<Out> Finalizer<Out> for NoFinalizer {
    fn finalize(&mut self, _: RcThreadSafety<Out>) -> impl Future<Output = ()> + SendSafety {
        ready(())
    }
}
