use crate::thread_safety::RcThreadSafety;

/// Called with a cached value when its container closes, as in Froodi.
pub trait Finalizer<Dep>: Clone + 'static {
    fn finalize(&mut self, dependency: RcThreadSafety<Dep>);
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

/// The finalizer slot of a registration: [`NoFinalizer`] or [`WithFinalizer`]. A registration
/// without a finalizer stores nothing.
pub trait MaybeFinalizer<Dep> {
    const PRESENT: bool;

    /// Runs the finalizer on a clone of itself, as Froodi does.
    fn finalize(&self, dependency: RcThreadSafety<Dep>);
}

#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct NoFinalizer;

impl<Dep> MaybeFinalizer<Dep> for NoFinalizer {
    const PRESENT: bool = false;

    #[inline]
    fn finalize(&self, _dependency: RcThreadSafety<Dep>) {}
}

#[doc(hidden)]
#[derive(Clone)]
pub struct WithFinalizer<F>(pub F);

impl<Dep, F: Finalizer<Dep>> MaybeFinalizer<Dep> for WithFinalizer<F> {
    const PRESENT: bool = true;

    #[inline]
    fn finalize(&self, dependency: RcThreadSafety<Dep>) {
        self.0.clone().finalize(dependency);
    }
}
