use alloc::{boxed::Box, sync::Arc};
use core::any::Any;

use crate::service::{service_fn, BoxCloneService};

pub trait Finalizer<Dep>: Clone + 'static {
    fn finalize(&mut self, dependency: Arc<Dep>);
}

pub(crate) type BoxedCloneFinalizer = BoxCloneService<Arc<dyn Any + Send + Sync>, (), ()>;

#[must_use]
pub(crate) fn boxed_finalizer_factory<Dep, Fin>(mut finalizer: Fin) -> BoxedCloneFinalizer
where
    Dep: Send + Sync + 'static,
    Fin: Finalizer<Dep> + Send + Sync,
{
    BoxCloneService(Box::new(service_fn(move |dependency: Arc<dyn Any + Send + Sync>| {
        let dependency = dependency.downcast::<Dep>().expect("Failed to downcast value in finalizer factory");
        finalizer.finalize(dependency);
        const { Ok(()) }
    })))
}

impl<F, Dep> Finalizer<Dep> for F
where
    F: FnMut(Arc<Dep>) + Clone + 'static,
{
    #[inline]
    fn finalize(&mut self, dependency: Arc<Dep>) {
        self(dependency);
    }
}
