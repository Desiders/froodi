use crate::thread_safety::RcThreadSafety;

/// Runs on values constructed through `get` when the container closes, even if caching is disabled.
pub trait Finalizer<Dep>: Clone + 'static {
    fn finalize(&mut self, dependency: RcThreadSafety<Dep>);
}

impl<Fin, Dep> Finalizer<Dep> for Fin
where
    Fin: FnMut(RcThreadSafety<Dep>) + Clone + 'static,
{
    #[inline]
    fn finalize(&mut self, dependency: RcThreadSafety<Dep>) {
        self(dependency);
    }
}

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
pub struct WithFinalizer<Fin>(pub Fin);

impl<Dep, Fin: Finalizer<Dep>> MaybeFinalizer<Dep> for WithFinalizer<Fin> {
    const PRESENT: bool = true;

    #[inline]
    fn finalize(&self, dependency: RcThreadSafety<Dep>) {
        self.0.clone().finalize(dependency);
    }
}
