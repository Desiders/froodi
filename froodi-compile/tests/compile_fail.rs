//! Compile errors of the typed registry, pinned for the default (`thread_safe`) build.
#![cfg(all(feature = "thread_safe", not(feature = "direct-edges")))]

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
    #[cfg(feature = "async")]
    t.compile_fail("tests/ui/async/*.rs");
}
