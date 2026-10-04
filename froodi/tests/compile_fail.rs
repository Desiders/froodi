//! Compile-time diagnostics through the public registration macros.
// Cargo subprocesses run outside Miri's interpreter.
#![cfg(not(miri))]

use std::{fs, path::PathBuf, process::Command};

#[test]
fn registration_errors() {
    // A fail-only trybuild suite uses checking; const cycles need code generation below.
    let cases = trybuild::TestCases::new();
    for case in [
        "missing",
        "forged_executor",
        "forged_index",
        "bare_resolver",
        "resolver_ambiguity",
        "unknown_clause",
        "typed_missing",
        "typed_boundaries",
        "typed_forged",
    ] {
        // rustc qualifies type paths differently when async types are also present.
        let prefix = if !cfg!(feature = "async")
            && matches!(
                case,
                "missing" | "bare_resolver" | "typed_missing" | "typed_boundaries" | "typed_forged"
            ) {
            "no_async_"
        } else {
            ""
        };
        cases.compile_fail(format!("tests/ui/registry/{prefix}{case}.rs"));
    }
    #[cfg(feature = "async")]
    for case in [
        "sync_async",
        "sync_transient_async",
        "typed_async_missing",
        "typed_async_in_sync",
        "sync_async_finalizer",
    ] {
        cases.compile_fail(format!("tests/ui/registry/{case}.rs"));
    }
}

#[test]
fn topology_errors() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/registry/uninstantiated.rs");
    cases.pass("tests/ui/registry/resolver_fallback.rs");
    cases.pass("tests/ui/registry/opaque_dag.rs");
    cases.pass("tests/ui/registry/static_scope_uninstantiated.rs");

    // Const validation requires code generation. Rustc's long inferred type traces
    // vary across toolchains, so assert the stable error code and diagnostic instead.
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project = std::env::temp_dir().join(format!("froodi-topology-{}", std::process::id()));
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| source.parent().unwrap().join("target"))
        .join("topology-tests");
    fs::create_dir_all(&project).unwrap();
    let features: Vec<_> = [
        (cfg!(feature = "std"), "std"),
        (cfg!(feature = "thread_safe"), "thread_safe"),
        (cfg!(feature = "async"), "async"),
    ]
    .into_iter()
    .filter_map(|(enabled, feature)| enabled.then_some(format!("{feature:?}")))
    .collect();
    let mut manifest = format!(
        "[package]\nname = \"froodi-topology-tests\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[workspace]\n[features]\nasync = []\n[dependencies]\nfroodi = {{ path = {source:?}, default-features = false, features = [{}] }}\n",
        features.join(",")
    );

    let mut failures = vec![
        ("self_cycle", "dependency cycle in closed static registry"),
        ("indirect", "dependency cycle in closed static registry"),
        ("named_cycle", "dependency cycle in closed static registry"),
        ("unrequested", "dependency cycle in closed static registry"),
        ("generic", "dependency cycle in closed static registry"),
        ("opaque_cycle", "dependency cycle in known static dependencies"),
        ("limit_sync_cycle", "dependency cycle in closed static registry"),
        ("static_scope", "incompatible static scopes: app -> request"),
        ("static_scope_generic", "incompatible static scopes: app -> request"),
        ("static_scope_transient", "incompatible static scopes: app -> request"),
        ("typed_cycle", "dependency cycle in closed static registry"),
    ];
    if cfg!(feature = "async") {
        failures.extend([
            ("async_cycle", "dependency cycle in closed static registry"),
            ("opaque_async_cycle", "dependency cycle in known static dependencies"),
            ("limit_async_cycle", "dependency cycle in closed static registry"),
            ("static_scope_async", "incompatible static scopes: app -> request"),
        ]);
    }
    for (name, _) in &failures {
        let path = source.join("tests/ui/registry").join(format!("{name}.rs"));
        manifest.push_str(&format!("\n[[bin]]\nname = {name:?}\npath = {path:?}\n"));
    }
    fs::write(project.join("Cargo.toml"), manifest).unwrap();

    let check = Command::new(env!("CARGO"))
        .current_dir(&project)
        .env("CARGO_TARGET_DIR", &target)
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_INCREMENTAL", "0")
        .args(["check", "--offline", "--bin", "self_cycle"])
        .args(cfg!(feature = "async").then_some("--features=async"))
        .output()
        .unwrap();
    assert!(check.status.success(), "cargo check:\n{}", String::from_utf8_lossy(&check.stderr));

    let output = Command::new(env!("CARGO"))
        .current_dir(&project)
        .env("CARGO_TARGET_DIR", &target)
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_INCREMENTAL", "0")
        .args(["build", "--offline", "--keep-going", "--bins", "--jobs", "2"])
        .args(cfg!(feature = "async").then_some("--features=async"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "cyclic and invalid-scope registries unexpectedly built");
    for (name, diagnostic) in failures {
        let path = format!("/tests/ui/registry/{name}.rs");
        assert!(
            stderr
                .split("error[E0080]:")
                .skip(1)
                .any(|error| error.contains(&path) && error.contains(diagnostic)),
            "missing expected error for {name}:\n{stderr}"
        );
    }

    fs::remove_dir_all(project).unwrap();
}
