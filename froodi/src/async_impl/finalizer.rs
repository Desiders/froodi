use alloc::{boxed::Box, sync::Arc};
use core::{any::Any, future::Future};

use super::service::{service_fn, BoxCloneService};

pub trait Finalizer<Dep>: Clone + 'static {
    fn finalize(&mut self, dependency: Arc<Dep>) -> impl Future<Output = ()> + Send;
}

pub(crate) type BoxedCloneFinalizer = BoxCloneService<Arc<dyn Any + Send + Sync>, (), ()>;

#[must_use]
pub(crate) fn boxed_finalizer_factory<Dep, Fin>(finalizer: Fin) -> BoxedCloneFinalizer
where
    Dep: Send + Sync + 'static,
    Fin: Finalizer<Dep> + Send + Sync,
{
    BoxCloneService::new(Box::new(service_fn(move |dependency: Arc<dyn Any + Send + Sync>| {
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
    F: FnMut(Arc<Dep>) -> Fut + Clone + 'static,
    Fut: Future<Output = ()> + Send,
{
    #[inline]
    fn finalize(&mut self, dependency: Arc<Dep>) -> impl Future<Output = ()> + Send {
        self(dependency)
    }
}
