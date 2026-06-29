//! Compile-fail (UI) tests for the `registry!` / `async_registry!` macro diagnostics, via
//! `trybuild`. Each fixture in `tests/ui/` deliberately misuses a macro; its `.stderr` snapshot
//! pins the exact `compile_error!` message produced for every error branch.
//!
//! Snapshots are toolchain-sensitive (they capture rustc's rendered diagnostic). Run these on a
//! stable toolchain and regenerate after a compiler/macro change with:
//!     TRYBUILD=overwrite cargo test -p froodi --test compile_fail
//! (add `--features async` to also refresh the async fixture).

#[test]
fn registry_macro_errors() {
    trybuild::TestCases::new().compile_fail("tests/ui/registry_errors.rs");
}

#[cfg(feature = "async")]
#[test]
fn async_registry_macro_errors() {
    trybuild::TestCases::new().compile_fail("tests/ui/async_registry_errors.rs");
}
