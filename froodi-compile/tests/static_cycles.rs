//! Const validation runs at monomorphization. trybuild's check-only failures cannot test it.
#![cfg(not(miri))]
use std::{path::PathBuf, process::Command};

#[test]
fn cycle_validation_compilation_stages() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/topology");
    let target = std::env::temp_dir().join(format!("froodi-topology-tests-{}", std::process::id()));
    let run = |command: &str, case: &str, success: bool| {
        let output = Command::new(env!("CARGO"))
            .current_dir(&dir)
            .env("CARGO_TARGET_DIR", &target)
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("CARGO_PROFILE_TEST_DEBUG", "0")
            .args([command, "--offline", "--bin", case])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.success(), success, "cargo {command} --bin {case}:\n{stderr}");
        if !success {
            assert!(stderr.contains("dependency cycle in closed static registry"), "{stderr}");
            assert!(stderr.contains("E0080"), "{stderr}");
        }
    };
    for case in [
        "dag",
        "uninstantiated",
        "self_cycle",
        "indirect",
        "unrequested",
        "generic",
        "async_cycle",
    ] {
        run("check", case, true);
        let valid = matches!(case, "dag" | "uninstantiated");
        run("build", case, valid);
        run("test", case, valid);
    }
    run("run", "dag", true);
    std::fs::remove_dir_all(target).unwrap();
}
