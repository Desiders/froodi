#![no_std]

extern crate alloc;

#[macro_use]
pub(crate) mod macros;

pub(crate) mod any;
pub(crate) mod cache;
pub(crate) mod container;
pub(crate) mod context;
pub(crate) mod dependency_resolver;
pub(crate) mod errors;
pub(crate) mod finalizer;
pub(crate) mod inject;
pub(crate) mod instantiator;
pub(crate) mod integrations;
pub(crate) mod registry;
pub(crate) mod scope;
pub(crate) mod service;
pub(crate) mod utils;

#[cfg(feature = "async")]
pub mod async_impl;

pub use container::Container;
pub use context::Context;
pub use errors::{InstantiateErrorKind, InstantiatorErrorKind, ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind};
pub use finalizer::Finalizer;
pub use inject::{Inject, InjectTransient};
pub use instantiator::{instance, Config};
pub use registry::RegistriesBuilder;
pub use scope::{DefaultScope, Scope, Scopes};

#[cfg(feature = "axum")]
pub use integrations::axum;
