//! Registration IR, graph compiler and diagnostics of the experimental compile-time Froodi engine.
//!
//! This crate knows nothing about `Any`, caches or containers. It receives a registry as a
//! [`Graph`] of Froodi registrations and produces a [`CompiledGraph`].
//!
//! Rust type identity is not reimplemented here. A frontend either passes edges rustc already
//! resolved ([`Target::Id`]) or opaque binding keys ([`Target::Key`]) that are only compared.
#![no_std]

extern crate alloc;

pub mod compiler;
pub mod diagnostic;
pub mod ir;

pub use compiler::{compile, CompiledEdge, CompiledGraph, CompiledNode, ScopeId};
pub use diagnostic::{Diagnostic, Diagnostics, PathStep};
pub use ir::{DependencyRequest, ExecutionKind, Graph, Origin, Registration, RegistrationId, RequestMode, ScopeKey, Target, ValueSource};
