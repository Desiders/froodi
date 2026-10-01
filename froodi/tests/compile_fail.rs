//! Compile-time diagnostics through the public registration macros.
#![cfg(not(miri))]

#[test]
fn registry_macro_errors() {
    trybuild::TestCases::new().compile_fail("tests/ui/registry_errors.rs");
}

#[cfg(feature = "async")]
#[test]
fn async_registry_macro_errors() {
    trybuild::TestCases::new().compile_fail("tests/ui/async_registry_errors.rs");
}

#[cfg(feature = "compiled")]
#[test]
fn compiled_registration_errors() {
    // A fail-only trybuild suite uses checking; const cycles need code generation below.
    let cases = trybuild::TestCases::new();
    for case in [
        "missing",
        "ambiguous",
        "forged_executor",
        "forged_index",
        "bare_resolver",
        "resolver_ambiguity",
        "unknown_clause",
        "invalid_instantiator",
    ] {
        // rustc qualifies type paths differently when async types are also present.
        let prefix = if !cfg!(feature = "async") && matches!(case, "missing" | "ambiguous" | "bare_resolver") {
            "no_async_"
        } else {
            ""
        };
        cases.compile_fail(format!("tests/ui/compiled/{prefix}{case}.rs"));
    }
    #[cfg(feature = "async")]
    for case in ["sync_async", "sync_transient_async", "invalid_async_instantiator"] {
        cases.compile_fail(format!("tests/ui/compiled/{case}.rs"));
    }
}

#[cfg(feature = "compiled")]
#[test]
fn compiled_cycle_errors() {
    let cases = trybuild::TestCases::new();
    // A pass case makes trybuild build this suite, evaluating generic const validation.
    cases.pass("tests/ui/compiled/uninstantiated.rs");
    cases.pass("tests/ui/compiled/resolver_fallback.rs");
    for case in [
        "self_cycle",
        "indirect",
        "named_cycle",
        "unrequested",
        "generic",
        "limit_sync_cycle",
    ] {
        cases.compile_fail(format!("tests/ui/compiled/{case}.rs"));
    }
    #[cfg(feature = "async")]
    for case in ["async_cycle", "limit_async_cycle"] {
        cases.compile_fail(format!("tests/ui/compiled/{case}.rs"));
    }
}
