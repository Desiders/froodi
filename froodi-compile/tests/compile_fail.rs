//! Compile errors of the typed registry, pinned for the default (`thread_safe`) build.
#![cfg(feature = "thread_safe")]

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
