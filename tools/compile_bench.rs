//! Run with: rustc --edition=2021 tools/compile_bench.rs -o /tmp/compile-bench
//! Then: /tmp/compile-bench <repo> <output-dir> [repetitions=3] [stages|full|all]
//! Generated app crates share dependency artifacts; only the app is cleaned per sample.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

fn run(cmd: &mut Command, log: &Path) -> f64 {
    let start = Instant::now();
    let output = cmd.output().expect("start command");
    let elapsed = start.elapsed().as_secs_f64();
    fs::write(log, &output.stderr).unwrap();
    assert!(output.status.success(), "command failed: {cmd:?}; see {}", log.display());
    elapsed
}

fn cargo(dir: &Path, target: &Path) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(dir)
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_INCREMENTAL", "1")
        .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
        .env("CARGO_PROFILE_RELEASE_DEBUG", "false");
    cmd
}

fn source(shape: &str, stage: &str, edit: &str) -> String {
    let count = if shape == "chain100" { 100 } else { 500 };
    let mut text = String::from("#![allow(unused_imports, dead_code)]\nuse froodi_compile::{registry, Container, Inject, InstantiateErrorKind, DefaultScope::App};\n");
    for i in 0..count {
        text.push_str(&format!("struct T{i}(usize);\n"));
        if i == 0 || shape == "flat500" {
            let value = if edit == "body" && i == 0 { 2 } else { 1 };
            text.push_str(&format!(
                "fn p{i}() -> Result<T{i}, InstantiateErrorKind> {{ Ok(T{i}({value})) }}\n"
            ));
        } else {
            text.push_str(&format!(
                "fn p{i}(dep: Inject<T{}>) -> Result<T{i}, InstantiateErrorKind> {{ Ok(T{i}(dep.0.0 + 1)) }}\n",
                i - 1
            ));
        }
    }
    text.push_str("fn main() { let registry = registry! {\n");
    for i in 0..count {
        text.push_str(&format!("provide(App, p{i}),\n"));
    }
    if edit == "topology" {
        text.push_str("provide(App, || Ok::<u64, InstantiateErrorKind>(1)),\n");
    }
    text.push_str("};\n");
    match stage {
        "typed" => text.push_str("std::hint::black_box(registry);\n"),
        "full" => text.push_str(&format!(
            "let c = Container::new(registry); std::hint::black_box(c.get::<T{}>().unwrap().0);\n",
            count - 1
        )),
        stage => text.push_str(&format!("froodi_compile_runtime::compile_bench::{stage}(registry);\n")),
    }
    text.push_str("}\n");
    text
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

fn main() {
    let args: Vec<_> = env::args().collect();
    assert!(
        args.len() >= 3,
        "usage: compile-bench <repo> <output-dir> [repetitions=3] [stages|full|all]"
    );
    let repo = fs::canonicalize(&args[1]).unwrap();
    let output = PathBuf::from(&args[2]);
    let repetitions = args.get(3).map_or(3, |v| v.parse::<usize>().unwrap());
    assert!(repetitions > 0);
    let mode = args.get(4).map_or("all", String::as_str);
    fs::create_dir_all(&output).unwrap();
    let output = fs::canonicalize(output).unwrap();
    let target = output.join("target");
    let version = Command::new("rustc").arg("-Vv").output().unwrap();
    fs::write(output.join("toolchain.txt"), version.stdout).unwrap();
    let mut results = String::from("shape,stage,measurement,seconds,bytes\n");
    for shape in ["chain100", "flat500"] {
        for stage in ["typed", "linking", "metadata", "executors", "full"] {
            if mode == "full" && stage != "full" {
                continue;
            }
            let dir = output.join(format!("{shape}-{stage}"));
            fs::create_dir_all(dir.join("src")).unwrap();
            fs::write(
                dir.join("Cargo.toml"),
                format!(
                    r#"[package]
name = "compile-bench-app"
version = "0.0.0"
edition = "2021"
[workspace]
[dependencies]
froodi-compile = {{ path = {:?} }}
froodi-compile-runtime = {{ path = {:?}, features = ["compile-bench"] }}
[profile.dev]
debug = "line-tables-only"
[profile.release]
debug = false
"#,
                    repo.join("froodi-compile"),
                    repo.join("froodi-compile-runtime")
                ),
            )
            .unwrap();
            let src = dir.join("src/main.rs");
            let original = source(shape, stage, "none");
            fs::write(&src, &original).unwrap();
            run(cargo(&dir, &target).args(["build", "--offline"]), &dir.join("warm.log"));
            for measurement in ["clean-app", "body", "topology", "release"] {
                if stage != "full" && measurement != "clean-app" {
                    continue;
                }
                if mode == "stages" && measurement != "clean-app" {
                    continue;
                }
                let mut samples = Vec::new();
                let mut bytes = 0;
                for sample in 0..repetitions {
                    fs::write(&src, &original).unwrap();
                    let release = measurement == "release";
                    let mut build = cargo(&dir, &target);
                    build.args(["build", "--offline"]);
                    if release {
                        build.arg("--release");
                    }
                    // Warm dependencies and establish the pre-edit incremental state.
                    run(&mut build, &dir.join("prepare.log"));
                    if measurement == "clean-app" || release {
                        let mut clean = cargo(&dir, &target);
                        clean.args(["clean", "-p", "compile-bench-app"]);
                        if release {
                            clean.arg("--release");
                        }
                        run(&mut clean, &dir.join("clean.log"));
                    } else {
                        fs::write(&src, source(shape, stage, measurement)).unwrap();
                    }
                    let seconds = run(&mut build, &dir.join(format!("{measurement}-{sample}.log")));
                    let binary = target.join(if release { "release" } else { "debug" }).join("compile-bench-app");
                    bytes = fs::metadata(&binary).unwrap().len();
                    if stage == "full" {
                        run(&mut Command::new(binary), &dir.join("run.log"));
                    }
                    samples.push(seconds);
                }
                let seconds = median(samples.clone());
                println!("{shape}/{stage}/{measurement}: {seconds:.3}s {bytes} bytes ({samples:?})");
                results.push_str(&format!("{shape},{stage},{measurement},{seconds:.6},{bytes}\n"));
                fs::write(output.join("results.csv"), &results).unwrap();
            }
            // Attribute compiler time separately from Cargo, dependency builds and linking.
            fs::write(&src, original).unwrap();
            let mut profile = cargo(&dir, &target);
            profile
                .env("RUSTC_BOOTSTRAP", "1")
                .args(["rustc", "--offline", "--", "-Ztime-passes"]);
            run(&mut profile, &dir.join("time-passes.log"));
        }
    }
}
