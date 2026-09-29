//! Experimental compile-time Froodi engine.
//!
//! The public surface mirrors Froodi: `registry!`, `Container`, `get`, `Inject` and the scope
//! types keep their names and meaning. Code written against `froodi` should compile against this
//! crate by changing the crate name in its imports.
#![cfg_attr(not(feature = "std"), no_std)]

pub use froodi_compile_macros::registry;
pub use froodi_compile_runtime::*;
