//! Runtime of the experimental compile-time Froodi engine.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[macro_use]
pub(crate) mod macros;

pub mod config;
pub mod container;
pub mod context;
pub mod dependency_resolver;
pub mod errors;
pub mod finalizer;
pub mod graph;
pub mod inject;
pub mod instantiator;
pub(crate) mod lock;
pub mod registry;
pub mod scope;
pub mod thread_safety;

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
pub use scope::{DefaultScope, Scope, ScopeData, Scopes};

#[doc(hidden)]
pub mod __private {
    pub use crate::finalizer::{NoFinalizer, WithFinalizer};
    pub use crate::graph::{Empty, Node, Reg};
    pub use crate::registry::{reg, Registry};
    pub use froodi_compile_core::{Origin, ValueSource};
}
