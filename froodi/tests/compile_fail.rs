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
        "typed_missing",
        "typed_boundaries",
        "typed_forged",
        "typed_variance",
    ] {
        // rustc qualifies type paths differently when async types are also present.
        let prefix = if !cfg!(feature = "async")
            && matches!(
                case,
                "missing" | "ambiguous" | "bare_resolver" | "typed_missing" | "typed_boundaries" | "typed_forged"
            ) {
            "no_async_"
        } else {
            ""
        };
        cases.compile_fail(format!("tests/ui/compiled/{prefix}{case}.rs"));
    }
    #[cfg(feature = "async")]
    for case in [
        "sync_async",
        "sync_transient_async",
        "invalid_async_instantiator",
        "typed_async_missing",
    ] {
        cases.compile_fail(format!("tests/ui/compiled/{case}.rs"));
    }
}

#[cfg(feature = "compiled")]
#[test]
fn compiled_topology_errors() {
    let cases = trybuild::TestCases::new();
    // A pass case makes trybuild build this suite, evaluating generic const validation.
    cases.pass("tests/ui/compiled/uninstantiated.rs");
    cases.pass("tests/ui/compiled/resolver_fallback.rs");
    cases.pass("tests/ui/compiled/opaque_dag.rs");
    cases.pass("tests/ui/compiled/static_scope_uninstantiated.rs");
    for case in [
        "self_cycle",
        "indirect",
        "named_cycle",
        "unrequested",
        "generic",
        "opaque_cycle",
        "limit_sync_cycle",
        "static_scope",
        "static_scope_generic",
        "static_scope_transient",
        "typed_cycle",
    ] {
        cases.compile_fail(format!("tests/ui/compiled/{case}.rs"));
    }
    #[cfg(feature = "async")]
    for case in ["async_cycle", "opaque_async_cycle", "limit_async_cycle", "static_scope_async"] {
        cases.compile_fail(format!("tests/ui/compiled/{case}.rs"));
    }
}
