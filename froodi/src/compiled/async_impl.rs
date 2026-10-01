//! Async adapters use the same provider witnesses and Froodi's async lifecycle.

mod registration;
mod registry;

pub use registration::async_reg;

pub(crate) use registry::{finish, prepare, IntoRegistry, Selected};
