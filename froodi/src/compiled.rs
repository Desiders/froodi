//! Opt-in typed registries executed by Froodi's existing container lifecycle.

#[macro_use]
mod tuples;

mod boundary;
mod dependency_resolver;
mod macros;
mod registration;
mod registry;
mod topology;

pub(crate) mod linking;

#[cfg(feature = "async")]
pub(crate) mod async_impl;

pub use boundary::{context, runtime};
pub use dependency_resolver::RuntimeDependency;
pub use registration::{reg, NoFinalizer};
pub use registry::{IntoFragment, Registry};

pub(crate) use registry::{finish, prepare, IntoRegistry, RegistrationId};

#[cfg(test)]
mod tests;
