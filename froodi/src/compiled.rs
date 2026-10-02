//! Opt-in typed registries executed by Froodi's existing container lifecycle.

#[macro_use]
mod tuples;

mod boundary;
mod dependency_resolver;
mod macros;
mod registration;
mod registry;
mod static_scope;
mod topology;

#[cfg(feature = "async")]
pub(crate) mod async_impl;
pub(crate) mod linking;

pub use boundary::{context, runtime};
pub use dependency_resolver::RuntimeDependency;
pub use registration::{reg, LocatedRegistration, NoFinalizer, RegistrationSource};
pub use registry::{IntoFragment, Registry};
pub use static_scope::{ClassifyScope, ScopeType};

pub(crate) use registry::{finish, prepare, IntoRegistry, RegistrationId};

#[cfg(test)]
mod tests;
