//! `Rc`/`Arc` and `Send`/`Sync` selection by the `thread_safe` feature, as in Froodi.

#[cfg(feature = "thread_safe")]
mod thread_safe {
    use alloc::sync::Arc;
    use core::any::Any;

    pub trait SendSafety: Send {}

    pub trait SyncSafety: Sync {}

    impl<T: Send> SendSafety for T {}

    impl<T: Sync> SyncSafety for T {}

    pub type RcThreadSafety<T> = Arc<T>;

    pub type RcAnyThreadSafety = RcThreadSafety<dyn Any + Send + Sync>;

    pub type BoxAnyThreadSafety = alloc::boxed::Box<dyn Any + Send + Sync>;
}

#[cfg(not(feature = "thread_safe"))]
mod thread_unsafe {
    use alloc::rc::Rc;
    use core::any::Any;

    pub trait SendSafety {}

    pub trait SyncSafety {}

    impl<T> SendSafety for T {}

    impl<T> SyncSafety for T {}

    pub type RcThreadSafety<T> = Rc<T>;

    pub type RcAnyThreadSafety = RcThreadSafety<dyn Any>;

    pub type BoxAnyThreadSafety = alloc::boxed::Box<dyn Any>;
}

#[cfg(feature = "thread_safe")]
pub use thread_safe::{BoxAnyThreadSafety, RcAnyThreadSafety, RcThreadSafety, SendSafety, SyncSafety};

#[cfg(not(feature = "thread_safe"))]
pub use thread_unsafe::{BoxAnyThreadSafety, RcAnyThreadSafety, RcThreadSafety, SendSafety, SyncSafety};

/// Recovers the typed pointer from an erased one whose type the compiled graph already fixed.
///
/// # Safety
/// `value` must have been created from an `RcThreadSafety<T>`.
#[inline]
pub(crate) unsafe fn downcast_unchecked<T: 'static>(value: RcAnyThreadSafety) -> RcThreadSafety<T> {
    debug_assert_eq!((*value).type_id(), core::any::TypeId::of::<T>());
    // SAFETY: the caller guarantees the allocation holds a `T`; the cast only drops the vtable.
    unsafe { RcThreadSafety::from_raw(RcThreadSafety::into_raw(value).cast::<T>()) }
}
