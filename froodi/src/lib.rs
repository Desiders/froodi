#![no_std]

extern crate alloc;

#[macro_use]
pub(crate) mod macros;

pub(crate) mod any;
pub(crate) mod cache;
pub(crate) mod config;
pub(crate) mod container;
pub(crate) mod context;
pub(crate) mod dependency;
pub(crate) mod dependency_resolver;
pub(crate) mod errors;
pub(crate) mod finalizer;
pub(crate) mod inject;
pub(crate) mod instantiator;
pub(crate) mod integrations;
pub(crate) mod registry;
pub(crate) mod scope;
pub(crate) mod service;

pub mod macros_utils;
pub mod utils;

#[cfg(feature = "async")]
pub mod async_impl;

pub use any::TypeInfo;
pub use config::Config;
pub use container::Container;
pub use context::Context;
pub use dependency::Dependency;
pub use dependency_resolver::DependencyResolver;
pub use errors::{InstantiateErrorKind, InstantiatorResult, ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind};
pub use finalizer::Finalizer;
pub use inject::{Inject, InjectTransient};
pub use instantiator::{instance, Instantiator};
pub use registry::{InstantiatorData, Registry};
pub use scope::{DefaultScope, Scope, Scopes};

#[cfg(feature = "axum")]
pub use integrations::axum;

#[cfg(feature = "dptree")]
pub use integrations::dptree;

#[cfg(feature = "telers")]
pub use integrations::telers;
