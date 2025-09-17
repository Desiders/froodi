use alloc::boxed::Box;

use crate::{
    service::{service_fn, BoxCloneService},
    utils::thread_safety::{RcAnyThreadSafety, RcThreadSafety, SendSafety, SyncSafety},
};

pub trait Finalizer<Dep>: Clone + 'static {
    fn finalize(&mut self, dependency: RcThreadSafety<Dep>);
}

pub(crate) type BoxedCloneFinalizer = BoxCloneService<RcAnyThreadSafety, (), ()>;

#[must_use]
pub(crate) fn boxed_finalizer_factory<Dep, Fin>(mut finalizer: Fin) -> BoxedCloneFinalizer
where
    Dep: SendSafety + SyncSafety + 'static,
    Fin: Finalizer<Dep> + SendSafety + SyncSafety,
{
    BoxCloneService(Box::new(service_fn(move |dependency: RcAnyThreadSafety| {
        let dependency = dependency.downcast::<Dep>().expect("Failed to downcast value in finalizer factory");
        finalizer.finalize(dependency);
        Ok(())
    })))
}

impl<F, Dep> Finalizer<Dep> for F
where
    F: FnMut(RcThreadSafety<Dep>) + Clone + 'static,
{
    #[inline]
    fn finalize(&mut self, dependency: RcThreadSafety<Dep>) {
        self(dependency);
    }
}
