//! Public API checks in independent Cargo dependency graphs.
#![cfg(not(miri))]

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

#[test]
fn downstream_frontends_and_feature_unification() {
    let native = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project = CargoProject::new("consumer");
    copy_fixture(&native.join("tests/fixtures/consumer"), &project.dir);

    // Enable compiled through a second dependency while the consumer's own feature stays disabled.
    let enabler = project.dir.join("enabler");
    fs::create_dir_all(enabler.join("src")).unwrap();
    let enabler_manifest = toml::toml! {
        [package]
        name = "compiled-enabler"
        version = "0.0.0"
        edition = "2021"

        [dependencies.froodi]
        version = "=1.0.0-beta.18"
        features = ["compiled"]
    };
    fs::write(enabler.join("Cargo.toml"), toml::to_string_pretty(&enabler_manifest).unwrap()).unwrap();
    let source: syn::File = syn::parse_quote! {
        use froodi as scope_api;
        pub use froodi::compiled_registry as registry;
        pub mod scopes;
    };
    fs::write(enabler.join("src/lib.rs"), prettyplease::unparse(&source)).unwrap();
    fs::copy(native.join("tests/ui/compiled/support/scopes.rs"), enabler.join("src/scopes.rs")).unwrap();

    let packaged = std::env::var_os("FROODI_PACKAGE_DIR").map(PathBuf::from);
    let (native, macros) = if let Some(packages) = &packaged {
        (packages.join("froodi-1.0.0-beta.18"), packages.join("froodi-macros-1.0.0"))
    } else {
        let macros = native.parent().unwrap().join("froodi-macros");
        (native, macros)
    };
    let manifest_path = project.dir.join("Cargo.toml");
    let mut manifest: toml::Table = toml::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    manifest.insert(
        "patch".into(),
        toml::Value::Table(toml::toml! {
            [crates-io.froodi]
            path = (native.to_str().unwrap())

            [crates-io.froodi-macros]
            path = (macros.to_str().unwrap())
        }),
    );
    fs::write(manifest_path, toml::to_string_pretty(&manifest).unwrap()).unwrap();

    for features in ["", "async", "unified,async", "compiled,async"] {
        let mut args = vec!["run", "--offline"];
        if !features.is_empty() {
            args.extend(["--features", features]);
        }
        project.success(&args);
    }
    project.success(&["check", "--offline", "--features", "unified", "--bin", "static-scope"]);
    for command in ["build", "test"] {
        let output = project.cargo(&[command, "--offline", "--features", "unified", "--bin", "static-scope"]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "cargo {command} unexpectedly accepted invalid static scopes"
        );
        assert!(stderr.contains("E0080"), "{stderr}");
        assert!(stderr.contains("incompatible static scopes: app -> request"), "{stderr}");
    }
    let output = project.success(&[
        "metadata",
        "--offline",
        "--format-version",
        "1",
        "--features",
        "compiled,async,unified",
    ]);
    let graph = String::from_utf8(output.stdout).unwrap();
    assert!(!graph.contains("froodi-compile"), "retired package in consumer graph");
    if packaged.is_some() {
        assert!(!graph.contains(env!("CARGO_MANIFEST_DIR")), "packaged graph refers to checkout");
    }
}

struct CargoProject {
    dir: PathBuf,
}

impl CargoProject {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("froodi-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    fn cargo(&self, args: &[&str]) -> Output {
        let output = Command::new(env!("CARGO"))
            .current_dir(&self.dir)
            .env("CARGO_TARGET_DIR", self.dir.join("target"))
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("CARGO_PROFILE_TEST_DEBUG", "0")
            .args(args)
            .output()
            .unwrap();
        fs::write(self.dir.join("last-command.log"), [&output.stdout[..], &output.stderr[..]].concat()).unwrap();
        output
    }

    fn success(&self, args: &[&str]) -> Output {
        let output = self.cargo(args);
        assert!(
            output.status.success(),
            "cargo {args:?}:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }
}

impl Drop for CargoProject {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            fs::remove_dir_all(&self.dir).unwrap();
        }
    }
}

fn copy_fixture(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == "target" || entry.file_name() == "Cargo.lock" {
            continue;
        }
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_fixture(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
