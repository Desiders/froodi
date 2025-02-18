#![no_std]

extern crate alloc;

#[macro_use]
pub(crate) mod macros;

pub(crate) mod container;
pub(crate) mod context;
pub(crate) mod dependency_resolver;
pub(crate) mod instantiator;
pub(crate) mod registry;
pub(crate) mod service;

#[cfg(feature = "async")]
pub(crate) mod r#async;
