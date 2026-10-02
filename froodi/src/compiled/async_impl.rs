//! Async adapters use the same provider witnesses and Froodi's async lifecycle.

mod container;
mod registration;
mod registry;

pub use container::{TypedContainer, TypedContainerExt};
pub use registration::async_reg;

pub(crate) use registry::{finish, prepare, IntoRegistry, Selected};
