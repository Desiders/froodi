#[cfg(any(feature = "thread_safe", feature = "async"))]
mod construction;
mod edges;
#[cfg(feature = "thread_safe")]
mod lifecycle;
#[cfg(not(feature = "thread_safe"))]
mod local;
