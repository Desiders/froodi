use super::{Instantiator, RegistrationInstantiator};
use crate::{
    compiled::RegistrationId,
    errors::InstantiatorErrorKind,
    utils::thread_safety::{RcThreadSafety, SendSafety, SyncSafety},
    Container, DependencyResolver, InstantiateErrorKind, ResolveErrorKind, TypeInfo,
};
use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};
use core::{any::Any, marker::PhantomData};

type InstError = InstantiatorErrorKind<ResolveErrorKind, InstantiateErrorKind>;

trait CompiledCall: SendSafety + SyncSafety {
    fn call(&self, container: &Container, edges: &[RegistrationId]) -> Result<Box<dyn Any>, InstError>;
}

struct InstAdapter<Inst, Deps> {
    inst: Inst,
    marker: PhantomData<fn() -> Deps>,
}

impl<Inst, Deps> CompiledCall for InstAdapter<Inst, Deps>
where
    Inst: Instantiator<Deps> + SendSafety + SyncSafety,
    Deps: DependencyResolver + 'static,
{
    fn call(&self, container: &Container, edges: &[RegistrationId]) -> Result<Box<dyn Any>, InstError> {
        self.inst
            .instantiate_compiled(container, edges)
            .map(|out| Box::new(out) as Box<dyn Any>)
    }
}

#[derive(Clone)]
pub(crate) struct ErasedInstantiator {
    inst: RcThreadSafety<dyn CompiledCall>,
    pub(crate) keys: RcThreadSafety<[TypeInfo]>,
    pub(crate) edges: RcThreadSafety<[RegistrationId]>,
}

impl RegistrationInstantiator {
    pub(crate) fn compiled<Inst, Deps>(inst: Inst, edges: Vec<RegistrationId>) -> Self
    where
        Inst: Instantiator<Deps> + SendSafety + SyncSafety,
        Deps: DependencyResolver + 'static,
    {
        let inst = RcThreadSafety::new(InstAdapter::<Inst, Deps> { inst, marker: PhantomData });
        Self::Compiled(ErasedInstantiator {
            inst,
            keys: Vec::new().into(),
            edges: edges.into(),
        })
    }
}

impl ErasedInstantiator {
    #[inline]
    pub(super) fn call(&self, container: Container) -> Result<Box<dyn Any>, InstError> {
        self.inst.call(&container, &self.edges)
    }

    pub(crate) fn remap(&mut self, ids: &BTreeMap<TypeInfo, RegistrationId>) {
        self.edges = self
            .keys
            .iter()
            .map(|key| *ids.get(key).expect("compiled dependency has no runtime provider"))
            .collect::<Vec<_>>()
            .into();
    }
}
