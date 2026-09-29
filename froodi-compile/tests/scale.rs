//! A 100-deep static chain under rustc's default `recursion_limit`: the absence of a
//! `#![recursion_limit]` attribute is the point, as in Froodi's `registry_scale` tests.
// With `direct-edges`, proving a 100-deep chain exceeds the default recursion limit.
#![cfg(all(feature = "thread_safe", not(feature = "direct-edges")))]

#[path = "../benches/support/graphs.rs"]
mod graphs;

use graphs::{compile_engine, S99, T99};

#[test]
fn resolves_a_100_deep_static_chain() {
    assert!(compile_engine::chain(true).get::<S99>().is_ok());
    assert!(compile_engine::transient_chain().get_transient::<T99>().is_ok());
}

#[test]
fn resolves_a_100_deep_runtime_chain() {
    assert!(compile_engine::indexed::chain(true).get::<S99>().is_ok());
}
