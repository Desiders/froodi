macro_rules! native_registry {
    ($($tokens:tt)*) => {
        crate::registry! { $($tokens)* }.into_registry()
    };
}

#[cfg(feature = "async")]
macro_rules! native_async_registry {
    ($($tokens:tt)*) => {
        crate::async_registry! { $($tokens)* }.into_async_registry()
    };
}

#[cfg(any(feature = "thread_safe", feature = "async"))]
mod construction;
mod edges;
mod expression_order;
#[cfg(feature = "thread_safe")]
mod lifecycle;
#[cfg(not(feature = "thread_safe"))]
mod local;
#[cfg(feature = "async")]
mod mixed_validation;
mod scopes;
mod typed_container;
