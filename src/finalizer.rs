use alloc::{boxed::Box, rc::Rc};
use core::any::Any;

use crate::service::{service_fn, BoxCloneService};

pub(crate) trait Finalizer<Dep>: Clone + 'static {
    fn finalize(&mut self, dependency: Rc<Dep>);
}

pub(crate) type BoxedCloneFinalizer = BoxCloneService<Rc<dyn Any>, (), ()>;

#[must_use]
pub(crate) fn boxed_finalizer_factory<Dep, Fin>(mut finalizer: Fin) -> BoxedCloneFinalizer
where
    Dep: 'static,
    Fin: Finalizer<Dep>,
{
    BoxCloneService(Box::new(service_fn(move |dependency: Rc<dyn Any>| {
        let dependency = dependency.downcast::<Dep>().expect("Failed to downcast value in finalizer factory");
        finalizer.finalize(dependency);
        const { Ok(()) }
    })))
}

impl<F, Dep> Finalizer<Dep> for F
where
    F: FnMut(Rc<Dep>) + Clone + 'static,
{
    #[inline]
    fn finalize(&mut self, dependency: Rc<Dep>) {
        self(dependency);
    }
}
