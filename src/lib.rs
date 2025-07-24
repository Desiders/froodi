#![no_std]

extern crate alloc;

#[macro_use]
pub(crate) mod macros;

pub(crate) mod container;
pub(crate) mod context;
pub(crate) mod dependency_resolver;
pub(crate) mod errors;
pub(crate) mod finalizer;
pub(crate) mod instantiator;
pub(crate) mod registry;
pub(crate) mod scope;
pub(crate) mod service;

#[cfg(feature = "async")]
pub(crate) mod r#async;

pub use container::{Container, ContainerShared};
pub use context::Context;
pub use errors::{InstantiateErrorKind, InstantiatorErrorKind, ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind};
pub use finalizer::Finalizer;
pub use instantiator::Config;
pub use registry::RegistriesBuilder;
pub use scope::{DefaultScope, Scope};
