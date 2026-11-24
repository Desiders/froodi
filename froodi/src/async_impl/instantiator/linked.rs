use super::{Instantiator, InstantiatorFuture, RegistrationInstantiator};
use crate::{
    async_impl::Container,
    registry::RegistrationId,
    utils::thread_safety::{RcThreadSafety, SendSafety, SyncSafety},
    DependencyResolver, TypeInfo,
};
use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};
use core::{any::Any, marker::PhantomData};

trait LinkedCall: SendSafety + SyncSafety {
    fn call<'a>(&'a self, container: &'a Container, edges: &'a [RegistrationId]) -> InstantiatorFuture<'a>;
}

struct InstAdapter<Inst, Deps> {
    inst: Inst,
    marker: PhantomData<fn() -> Deps>,
}

impl<Inst, Deps> LinkedCall for InstAdapter<Inst, Deps>
where
    Inst: Instantiator<Deps> + SendSafety + SyncSafety,
    Deps: DependencyResolver + SendSafety + 'static,
{
    fn call<'a>(&'a self, container: &'a Container, edges: &'a [RegistrationId]) -> InstantiatorFuture<'a> {
        let future = self.inst.instantiate_linked(container, edges);
        Box::pin(async move { future.await.map(|out| Box::new(out) as Box<dyn Any>) })
    }
}

#[derive(Clone)]
pub(crate) struct ErasedInstantiator {
    inst: RcThreadSafety<dyn LinkedCall>,
    pub(crate) keys: RcThreadSafety<[TypeInfo]>,
    pub(crate) edges: RcThreadSafety<[RegistrationId]>,
}

impl RegistrationInstantiator {
    pub(crate) fn linked<Inst, Deps>(inst: Inst, edges: Vec<RegistrationId>) -> Self
    where
        Inst: Instantiator<Deps> + SendSafety + SyncSafety,
        Deps: DependencyResolver + SendSafety + 'static,
    {
        let inst = RcThreadSafety::new(InstAdapter::<Inst, Deps> { inst, marker: PhantomData });
        Self::Linked(ErasedInstantiator {
            inst,
            keys: Vec::new().into(),
            edges: edges.into(),
        })
    }
}

impl ErasedInstantiator {
    #[inline]
    pub(super) fn call<'a>(&'a self, container: &'a Container) -> InstantiatorFuture<'a> {
        self.inst.call(container, &self.edges)
    }

    pub(crate) fn remap(&mut self, ids: &BTreeMap<TypeInfo, RegistrationId>) {
        self.edges = self
            .keys
            .iter()
            .map(|key| *ids.get(key).expect("linked dependency has no runtime provider"))
            .collect::<Vec<_>>()
            .into();
    }
}
