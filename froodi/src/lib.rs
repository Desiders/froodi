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
pub(crate) mod instantiator;
pub(crate) mod integrations;
pub(crate) mod registry;
pub(crate) mod scope;
pub(crate) mod service;

pub use container::Container;
pub use context::Context;
pub use dependency_resolver::{Inject, InjectTransient};
pub use errors::{InstantiateErrorKind, InstantiatorErrorKind, ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind};
pub use finalizer::Finalizer;
pub use instantiator::{instance, Config};
pub use registry::RegistriesBuilder;
pub use scope::{DefaultScope, Scope, Scopes};

#[cfg(feature = "axum")]
pub use integrations::axum::{setup as setup_axum, setup_default as setup_axum_default, InjectErrorKind as AxumInjectErrorKind};
