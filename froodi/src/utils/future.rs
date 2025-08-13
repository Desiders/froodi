use alloc::boxed::Box;
use core::{future::Future, pin::Pin};

pub(crate) type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
