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
}

#[cfg(feature = "thread_safe")]
pub use thread_safe::RcThreadSafety;
#[cfg(feature = "thread_safe")]
pub(crate) use thread_safe::{RcAnyThreadSafety, SendSafety, SyncSafety};

#[cfg(not(feature = "thread_safe"))]
pub use thread_unsafe::RcThreadSafety;
#[cfg(not(feature = "thread_safe"))]
pub(crate) use thread_unsafe::{RcAnyThreadSafety, SendSafety, SyncSafety};
