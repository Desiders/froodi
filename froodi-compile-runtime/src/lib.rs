//! Runtime of the experimental compile-time Froodi engine.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[macro_use]
pub(crate) mod macros;

pub(crate) mod boundary;
pub(crate) mod config;
pub(crate) mod container;
pub(crate) mod context;
pub(crate) mod dependency_resolver;
pub(crate) mod errors;
pub(crate) mod finalizer;
pub(crate) mod graph;
pub(crate) mod inject;
pub(crate) mod instantiator;
pub(crate) mod lock;
pub(crate) mod registry;
pub(crate) mod runtime_registry;
pub(crate) mod scope;
pub mod thread_safety;

#[cfg(feature = "compile-bench")]
#[doc(hidden)]
pub mod compile_bench;

#[cfg(feature = "async")]
pub mod async_impl;

pub use boundary::{context, runtime};
pub use config::Config;
pub use container::Container;
pub use context::Context;
pub use dependency_resolver::DependencyResolver;
pub use errors::{
    InstantiateErrorKind, InstantiatorErrorKind, InstantiatorResult, ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind, TypeInfo,
};
pub use finalizer::Finalizer;
pub use inject::{Inject, InjectTransient};
pub use instantiator::{instance, Instantiator};
pub use registry::Registry;
pub use runtime_registry::RuntimeRegistry;
pub use scope::{DefaultScope, Scope, ScopeData, Scopes};

#[doc(hidden)]
pub mod __private {
    #[cfg(feature = "async")]
    pub use crate::async_impl::async_reg;
    pub use crate::finalizer::{NoFinalizer, WithFinalizer};
    pub use crate::graph::{Empty, Node, Reg};
    pub use crate::registry::{reg, Registry};
    pub use crate::runtime_registry::IntoFragment;
    pub use froodi_compile_core::{Origin, ValueSource};
}
