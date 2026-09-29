//! Home of the `build.rs` staging experiments of the experimental compile-time Froodi engine.
//!
//! A `build.rs` runs before its own crate is compiled, so it can read source files but not the
//! types rustc infers for them. Experiments here measure what a build-time step can still
//! contribute to the registry graph under that constraint.
