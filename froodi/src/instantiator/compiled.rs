use super::{Instantiator, RegistrationInstantiator};
use crate::{
    compiled::RegistrationId,
    errors::InstantiatorErrorKind,
    utils::thread_safety::{RcAnyThreadSafety, RcThreadSafety, SendSafety, SyncSafety},
    Container, DependencyResolver, InstantiateErrorKind, ResolveErrorKind, TypeInfo,
};
use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};
use core::any::Any;

type InstError = InstantiatorErrorKind<ResolveErrorKind, InstantiateErrorKind>;

/// Erasure contract (also used by the async adapter):
/// - Allocate Inst in its final Rc/Arc before deriving item; retain owner through every call/future.
/// - Pair item only with construct::<Inst, Deps>; safe callers cannot create this pairing.
/// - Preserve parameter order and repeats; opaque parameters consume no edge.
/// - Remap edges by exact TypeId after composition, then freeze the table. Each ID selects
///   the complete winning registration, including scope, cache policy and finalizer.
/// - Froodi checks cache/transient downcasts; Box owns aligned, initialized transient output.
///   Finalizers use the winning registration and receive its recorded values.
#[derive(Clone)]
pub(crate) struct ErasedInstantiator {
    owner: RcAnyThreadSafety,
    item: *const (),
    #[allow(clippy::type_complexity)]
    construct: unsafe fn(*const (), &Container, &[RegistrationId]) -> Result<Box<dyn Any>, InstError>,
    pub(crate) keys: RcThreadSafety<[TypeInfo]>,
    pub(crate) edges: RcThreadSafety<[RegistrationId]>,
}

// SAFETY: the pointer only addresses its immutable Send + Sync owner, and the function is paired with its type.
#[cfg(feature = "thread_safe")]
unsafe impl Send for ErasedInstantiator {}

#[cfg(feature = "thread_safe")]
unsafe impl Sync for ErasedInstantiator {}

impl RegistrationInstantiator {
    /// # Safety
    /// Edges must preserve the linked parameter types/order and be remapped before execution.
    pub(crate) unsafe fn compiled<Inst, Deps>(inst: Inst, edges: Vec<RegistrationId>) -> Self
    where
        Inst: Instantiator<Deps> + SendSafety + SyncSafety,
        Deps: DependencyResolver + 'static,
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
    pub(super) fn call(&self, container: Container) -> Result<Box<dyn Any>, InstError> {
        let _keep_alive = &self.owner;
        // SAFETY: construction is paired with the live item and final-registry edge order.
        unsafe { (self.construct)(self.item, &container, &self.edges) }
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

unsafe fn construct<Inst, Deps>(item: *const (), container: &Container, edges: &[RegistrationId]) -> Result<Box<dyn Any>, InstError>
where
    Inst: Instantiator<Deps>,
    Deps: DependencyResolver,
{
    // SAFETY: ErasedInstantiator pairs this function with an owned Inst allocation.
    let inst = unsafe { &*item.cast::<Inst>() };
    inst.instantiate_compiled(container, edges).map(|out| Box::new(out) as Box<dyn Any>)
}
