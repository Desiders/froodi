//! rustc --edition=2021 tools/compile_bench.rs -o /tmp/froodi-build-bench
//! /tmp/froodi-build-bench <repo> <fresh-output> [samples=3] [dynamic|compiled|both] [measurement] [default|runtime|static] [shapes]
//! Only app artifacts are cleaned; dependencies stay warm. All generated files are disposable.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

fn run(command: &mut Command, log: &Path) -> f64 {
    let start = Instant::now();
    let output = command.output().expect("start command");
    let seconds = start.elapsed().as_secs_f64();
    fs::write(log, [&output.stdout[..], &output.stderr[..]].concat()).unwrap();
    assert!(output.status.success(), "{command:?} failed; see {}", log.display());
    seconds
}

fn cargo(dir: &Path, target: &Path) -> Command {
    let mut command = Command::new("cargo");
    if dir.join("Cargo.lock").exists() {
        command.arg("--locked");
    }
    command
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_INCREMENTAL", "1")
        .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
        .env("CARGO_PROFILE_RELEASE_DEBUG", "false");
    command
}

fn source(shape: &str, edit: &str, engine: &str, scope: &str) -> String {
    let count = if shape == "chain100" { 100 } else { 500 };
    let registry = if engine == "compiled" {
        "froodi::compiled_registry"
    } else {
        "froodi::registry"
    };
    let mut text = format!("#![allow(unused_imports, dead_code)]\nuse {registry} as registry;\nuse froodi::{{Container, Inject, InstantiateErrorKind, DefaultScope::App}};\n");
    if scope != "default" {
        text.push_str(
            r#"
use froodi::{Scope, ScopeData, Scopes};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct BenchScope;

impl From<BenchScope> for ScopeData {
    fn from(_: BenchScope) -> Self {
        Self { name: "bench", priority: 1, is_skipped_by_default: false }
    }
}

impl Scopes<0> for BenchScope {
    type Scope = Self;

    fn all() -> (Self, [Self; 0]) { (BenchScope, []) }
}
"#,
        );
        if scope == "static" {
            text.push_str(
                r#"
impl froodi::StaticScope for BenchScope {
    const DATA: ScopeData = ScopeData { name: "bench", priority: 1, is_skipped_by_default: false };
}
"#,
            );
        } else {
            text.push_str(
                r#"
impl Scope for BenchScope {
    fn name(&self) -> &'static str { "bench" }

    fn priority(&self) -> u8 { 1 }
}
"#,
            );
        }
    }
    for index in 0..count {
        text.push_str(&format!("struct T{index}(usize);\n"));
        if index == 0 || shape == "flat500" {
            let value = if edit == "body" && index == 0 { 2 } else { 1 };
            text.push_str(&format!(
                "fn p{index}() -> Result<T{index}, InstantiateErrorKind> {{ Ok(T{index}({value})) }}\n"
            ));
        } else if shape == "chain100" && scope == "default" {
            text.push_str(&format!(
                "fn p{index}(dep: Inject<T{}>) -> Result<T{index}, InstantiateErrorKind> {{ Ok(T{index}(dep.0.0 + 1)) }}\n",
                index - 1
            ));
        } else {
            let edges = if shape == "edges2000" { index.min(4) } else { 1 };
            let parameters = (1..=edges)
                .map(|offset| format!("dep{offset}: Inject<T{}>", index - offset))
                .collect::<Vec<_>>()
                .join(", ");
            let work = (1..=edges)
                .map(|offset| format!("std::hint::black_box(&dep{offset}.0);"))
                .collect::<String>();
            text.push_str(&format!(
                "fn p{index}({parameters}) -> Result<T{index}, InstantiateErrorKind> {{ {work} Ok(T{index}(1)) }}\n"
            ));
        }
    }
    text.push_str("fn main() { let registry = registry! {\n");
    let value = if scope == "default" { "App" } else { "BenchScope" };
    for index in 0..count {
        text.push_str(&format!("provide({value}, p{index}),\n"));
    }
    if edit == "topology" {
        text.push_str(&format!("provide({value}, || Ok::<u64, InstantiateErrorKind>(1)),\n"));
    }
    text.push_str(&format!(
        "}}; let container = Container::new(registry); std::hint::black_box(container.get::<T{}>().unwrap().0); }}\n",
        count - 1
    ));
    text
}

fn main() {
    let args: Vec<_> = env::args().collect();
    assert!(
        args.len() >= 3,
        "usage: compile-bench <repo> <fresh-output> [samples=3] [dynamic|compiled|both] [measurement] [default|runtime|static] [shapes]"
    );
    let repo = fs::canonicalize(&args[1]).unwrap();
    let output = PathBuf::from(&args[2]);
    assert!(!output.exists(), "use a fresh output directory");
    fs::create_dir_all(&output).unwrap();
    let output = fs::canonicalize(output).unwrap();
    let repetitions = args.get(3).map_or(3, |value| value.parse::<usize>().unwrap());
    assert!(repetitions > 0);
    let selected = args.get(4).map_or("both", String::as_str);
    assert!(matches!(selected, "dynamic" | "compiled" | "both"));
    let only = args.get(5).map(String::as_str);
    let scope = args.get(6).map_or("default", String::as_str);
    assert!(matches!(scope, "default" | "runtime" | "static"));
    assert!(
        scope != "static" || selected == "compiled",
        "static scopes require the compiled feature"
    );
    let shapes = args.get(7).map_or("chain100,flat500", String::as_str);
    assert!(shapes
        .split(',')
        .all(|shape| matches!(shape, "chain100" | "flat500" | "edges500" | "edges2000")));
    assert!(only.is_none_or(|only| only
        .split(',')
        .all(|measurement| matches!(measurement, "clean-app" | "body" | "topology" | "release"))));
    let target = output.join("target");
    let version = Command::new("rustc").arg("-Vv").output().unwrap();
    fs::write(output.join("toolchain.txt"), version.stdout).unwrap();
    let mut results = String::from("engine,shape,measurement,seconds,bytes\n");
    for engine in ["dynamic", "compiled"] {
        if selected != "both" && selected != engine {
            continue;
        }
        for shape in shapes.split(',') {
            let dir = output.join(format!("{engine}-{shape}"));
            fs::create_dir_all(dir.join("src")).unwrap();
            let features = if engine == "compiled" { "[\"compiled\"]" } else { "[]" };
            fs::write(dir.join("Cargo.toml"), format!("[package]\nname = \"compile-bench-app\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[workspace]\n[dependencies]\nfroodi = {{ path = {:?}, features = {features} }}\n", repo.join("froodi"))).unwrap();
            let src = dir.join("src/main.rs");
            let original = source(shape, "none", engine, scope);
            fs::write(&src, &original).unwrap();
            run(cargo(&dir, &target).args(["build", "--offline"]), &dir.join("warm.log"));
            for measurement in ["clean-app", "body", "topology", "release"] {
                if only.is_some_and(|only| !only.split(',').any(|selected| selected == measurement)) {
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
                    run(&mut build, &dir.join("prepare.log"));
                    if measurement == "clean-app" || release {
                        let mut clean = cargo(&dir, &target);
                        clean.args(["clean", "-p", "compile-bench-app"]);
                        if release {
                            clean.arg("--release");
                        }
                        run(&mut clean, &dir.join("clean.log"));
                    } else {
                        fs::write(&src, source(shape, measurement, engine, scope)).unwrap();
                    }
                    samples.push(run(&mut build, &dir.join(format!("{measurement}-{sample}.log"))));
                    let binary = target.join(if release { "release" } else { "debug" }).join("compile-bench-app");
                    bytes = fs::metadata(&binary).unwrap().len();
                    run(&mut Command::new(binary), &dir.join("run.log"));
                }
                let mut ordered = samples.clone();
                ordered.sort_by(f64::total_cmp);
                let seconds = ordered[ordered.len() / 2];
                println!("{engine}/{shape}/{measurement}: {seconds:.3}s {bytes} bytes ({samples:?})");
                results.push_str(&format!("{engine},{shape},{measurement},{seconds:.6},{bytes}\n"));
                fs::write(output.join("results.csv"), &results).unwrap();
            }
        }
    }
}
