use super::{Instantiator, RegistrationInstantiator};
use crate::{
    async_impl::Container,
    compiled::RegistrationId,
    errors::{InstantiateErrorKind, InstantiatorErrorKind},
    utils::{
        future::BoxFuture,
        thread_safety::{RcAnyThreadSafety, RcThreadSafety, SendSafety, SyncSafety},
    },
    DependencyResolver, ResolveErrorKind, TypeInfo,
};
use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};
use core::any::Any;

type InstError = InstantiatorErrorKind<ResolveErrorKind, InstantiateErrorKind>;

/// Uses the sync erasure contract; calls borrow the owner until their futures complete.
#[derive(Clone)]
pub(crate) struct ErasedInstantiator {
    owner: RcAnyThreadSafety,
    item: *const (),
    #[allow(clippy::type_complexity)]
    construct: for<'a> unsafe fn(*const (), &'a Container, &'a [RegistrationId]) -> BoxFuture<'a, Result<Box<dyn Any>, InstError>>,
    pub(crate) keys: RcThreadSafety<[TypeInfo]>,
    pub(crate) edges: RcThreadSafety<[RegistrationId]>,
}

// SAFETY: item points into its immutable Send + Sync owner; futures borrow this owner until completion.
#[cfg(feature = "thread_safe")]
unsafe impl Send for ErasedInstantiator {}

#[cfg(feature = "thread_safe")]
unsafe impl Sync for ErasedInstantiator {}

impl RegistrationInstantiator {
    /// # Safety
    /// Edges must follow the instantiator parameter order, skipping runtime dependencies, and select
    /// registrations providing the exact injected Rust types. Assembly must preserve this pairing when remapping IDs.
    pub(crate) unsafe fn compiled<Inst, Deps>(inst: Inst, edges: Vec<RegistrationId>) -> Self
    where
        Inst: Instantiator<Deps> + SendSafety + SyncSafety,
        Deps: DependencyResolver + SendSafety + 'static,
    {
        let owner = RcThreadSafety::new(inst);
        let item = RcThreadSafety::as_ptr(&owner).cast();
        Self::Compiled(ErasedInstantiator {
            owner,
            item,
            construct: construct::<Inst, Deps>,
            keys: Vec::new().into(),
            edges: edges.into(),
        })
    }
}

impl ErasedInstantiator {
    pub(super) async fn call(&self, container: Container) -> Result<Box<dyn Any>, InstError> {
        let _keep_alive = &self.owner;
        // SAFETY: private construction pairs item/function and final-registry edges.
        unsafe { (self.construct)(self.item, &container, &self.edges) }.await
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

unsafe fn construct<'a, Inst, Deps>(
    item: *const (),
    container: &'a Container,
    edges: &'a [RegistrationId],
) -> BoxFuture<'a, Result<Box<dyn Any>, InstError>>
where
    Inst: Instantiator<Deps> + SendSafety + SyncSafety,
    Deps: DependencyResolver + SendSafety + 'static,
{
    // SAFETY: item is the owned Inst paired with this function, retained through the borrowing future.
    let inst = unsafe { &*item.cast::<Inst>() };
    let future = inst.instantiate_compiled(container, edges);
    Box::pin(async move { future.await.map(|out| Box::new(out) as Box<dyn Any>) })
}
