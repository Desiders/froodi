extern crate std;

use core::cell::Cell;
use std::thread_local;

thread_local! {
    pub(super) static CYCLE_CHECKS: Cell<usize> = const { Cell::new(0) };
}

macro_rules! native_registry {
    ($($tokens:tt)*) => {
        crate::registry! { $($tokens)* }.into_registry()
    };
}

#[cfg(feature = "async")]
macro_rules! native_async_registry {
    ($($tokens:tt)*) => {
        crate::registry! { $($tokens)* }.into_registry()
    };
}

#[cfg(any(feature = "thread_safe", feature = "async"))]
mod construction;
#[cfg(all(feature = "async", not(miri)))]
mod deep_async;
mod edges;
mod expression_order;
mod fragments;
#[cfg(feature = "thread_safe")]
mod lifecycle;
#[cfg(not(feature = "thread_safe"))]
mod local;
#[cfg(all(
    feature = "std",
    feature = "thread_safe",
    not(feature = "lock-parking-lot"),
    not(feature = "lock-spin")
))]
mod locks;
#[cfg(feature = "async")]
mod mixed_validation;
mod scopes;
mod typed_container;
