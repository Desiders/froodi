use alloc::boxed::Box;
use core::{future::Future, pin::Pin};

#[cfg(feature = "thread_safe")]
pub(crate) type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[cfg(not(feature = "thread_safe"))]
pub(crate) type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;
