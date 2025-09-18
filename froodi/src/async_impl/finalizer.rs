use alloc::boxed::Box;
use core::future::Future;

use super::service::{service_fn, BoxCloneService};
use crate::utils::thread_safety::{RcAnyThreadSafety, RcThreadSafety, SendSafety, SyncSafety};

pub trait Finalizer<Dep>: Clone + 'static {
    fn finalize(&mut self, dependency: RcThreadSafety<Dep>) -> impl Future<Output = ()> + SendSafety;
}

pub(crate) type BoxedCloneFinalizer = BoxCloneService<RcAnyThreadSafety, (), ()>;

#[must_use]
pub(crate) fn boxed_finalizer_factory<Dep, Fin>(finalizer: Fin) -> BoxedCloneFinalizer
where
    Dep: SendSafety + SyncSafety + 'static,
    Fin: Finalizer<Dep> + SendSafety + SyncSafety,
{
    BoxCloneService::new(Box::new(service_fn(move |dependency: RcAnyThreadSafety| {
        let mut finalizer = finalizer.clone();
        let dependency = dependency.downcast::<Dep>().expect("Failed to downcast value in finalizer factory");

        async move {
            finalizer.finalize(dependency).await;
            Ok(())
        }
    })))
}

impl<F, Fut, Dep> Finalizer<Dep> for F
where
    F: FnMut(RcThreadSafety<Dep>) -> Fut + Clone + 'static,
    Fut: Future<Output = ()> + SendSafety,
{
    #[inline]
    fn finalize(&mut self, dependency: RcThreadSafety<Dep>) -> impl Future<Output = ()> + SendSafety {
        self(dependency)
    }
}
