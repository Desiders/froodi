//! rustc --edition=2021 tools/compile_bench.rs -o /tmp/froodi-build-bench
//! /tmp/froodi-build-bench <repo> <fresh-output> [samples=3] [measurement] [default|runtime|static] [shapes] [sync|mixed] [provide|construct|alternating]
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

fn construction_source(count: usize, edit: &str, construction: &str) -> String {
    let mut text = String::from(
        "#![allow(dead_code)]\nuse froodi::{registry, Container, Inject, InjectTransient, InstantiateErrorKind, DefaultScope::App};\nuse std::sync::Arc;\nstruct Repository(u32);\nstruct RequestId(u32);\n",
    );
    for index in 0..count {
        let derived = construction == "construct" || (construction == "alternating" && index % 2 == 0);
        if derived {
            text.push_str("#[derive(froodi::Construct)]\n");
        }
        let attribute = if derived { "#[di(inject_transient)] " } else { "" };
        let extra = if edit == "body" && index == 0 {
            if derived {
                "#[di(inject_transient)] extra: RequestId,"
            } else {
                "extra: RequestId,"
            }
        } else {
            ""
        };
        text.push_str(&format!(
            "struct Service{index} {{ repository: Arc<Repository>, {attribute}request_id: RequestId, {extra} }}\n"
        ));
    }
    text.push_str("fn main() { let registry = registry! { scope(App) [\nprovide(|| Ok::<_, InstantiateErrorKind>(Repository(7))),\nprovide(|| Ok::<_, InstantiateErrorKind>(RequestId(11))),\n");
    for index in 0..count {
        let derived = construction == "construct" || (construction == "alternating" && index % 2 == 0);
        if derived {
            text.push_str(&format!("construct::<Service{index}>(),\n"));
        } else {
            let extra_parameter = if edit == "body" && index == 0 {
                ", InjectTransient(extra): InjectTransient<RequestId>"
            } else {
                ""
            };
            let extra_field = if edit == "body" && index == 0 { ", extra" } else { "" };
            text.push_str(&format!("provide(|Inject(repository): Inject<Repository>, InjectTransient(request_id): InjectTransient<RequestId>{extra_parameter}| Ok::<_, InstantiateErrorKind>(Service{index} {{ repository, request_id{extra_field} }})),\n"));
        }
    }
    if edit == "topology" {
        text.push_str("provide(|| Ok::<_, InstantiateErrorKind>(true)),\n");
    }
    text.push_str(&format!("] }}; let container = Container::new(registry); let service = container.get::<Service{}>().unwrap(); std::hint::black_box((service.repository.0, service.request_id.0)); }}\n", count - 1));
    text
}

fn source(shape: &str, edit: &str, scope: &str, mixed: bool, construction: &str) -> String {
    if matches!(shape, "fields100" | "fields500") {
        return construction_source(if shape == "fields100" { 100 } else { 500 }, edit, construction);
    }
    let count = if shape == "chain100" { 100 } else { 500 };
    let mut text = String::from(
        "#![allow(unused_imports, dead_code)]\nuse froodi::{registry, Container, Inject, InstantiateErrorKind, DefaultScope::App};\n",
    );
    if mixed {
        text.push_str("use froodi::async_impl::Container as AsyncContainer;\n");
    }
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
        let execution = if mixed && index % 2 == 1 { "async " } else { "" };
        text.push_str(&format!("struct T{index}(usize);\n"));
        if index == 0 || shape == "flat500" {
            let value = if edit == "body" && index == 0 { 2 } else { 1 };
            text.push_str(&format!(
                "{execution}fn p{index}() -> Result<T{index}, InstantiateErrorKind> {{ Ok(T{index}({value})) }}\n"
            ));
        } else if mixed && shape == "chain100" {
            // Alternating declaration kinds form two chains. A literal alternating
            // dependency chain would require forbidden sync -> async edges.
            let previous = if index == 1 { 0 } else { index - 2 };
            text.push_str(&format!(
                "{execution}fn p{index}(dep: Inject<T{previous}>) -> Result<T{index}, InstantiateErrorKind> {{ Ok(T{index}(dep.0.0 + 1)) }}\n"
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
    text.push_str("fn main() { let sync_fragment = registry! {\n");
    let value = if scope == "default" { "App" } else { "BenchScope" };
    for index in 0..count {
        if !mixed || index % 2 == 0 {
            text.push_str(&format!("provide({value}, p{index}),\n"));
        }
    }
    if mixed {
        text.push_str("}; let registry = registry! {\n");
        for index in (1..count).step_by(2) {
            text.push_str(&format!("provide({value}, p{index}),\n"));
        }
        text.push_str("extend(sync_fragment),\n");
    }
    if edit == "topology" {
        let execution = if mixed { "async " } else { "" };
        text.push_str(&format!("provide({value}, {execution}|| Ok::<u64, InstantiateErrorKind>(1)),\n"));
    }
    if mixed {
        text.push_str(&format!(
            "}}; tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {{ let container = AsyncContainer::new(registry); std::hint::black_box(container.get::<T{}>().await.unwrap().0); }}); }}\n", count - 1
        ));
    } else {
        text.push_str(&format!(
            "}}; let container = Container::new(sync_fragment); std::hint::black_box(container.get::<T{}>().unwrap().0); }}\n",
            count - 1
        ));
    }
    text
}

fn main() {
    let args: Vec<_> = env::args().collect();
    assert!(
        args.len() >= 3,
        "usage: compile-bench <repo> <fresh-output> [samples=3] [measurement] [default|runtime|static] [shapes] [sync|mixed] [provide|construct|alternating]"
    );
    let repo = fs::canonicalize(&args[1]).unwrap();
    let output = PathBuf::from(&args[2]);
    assert!(!output.exists(), "use a fresh output directory");
    fs::create_dir_all(&output).unwrap();
    let output = fs::canonicalize(output).unwrap();
    let repetitions = args.get(3).map_or(3, |value| value.parse::<usize>().unwrap());
    assert!(repetitions > 0);
    let only = args.get(4).map(String::as_str);
    let scope = args.get(5).map_or("default", String::as_str);
    assert!(matches!(scope, "default" | "runtime" | "static"));
    let shapes = args.get(6).map_or("chain100,flat500", String::as_str);
    let execution = args.get(7).map_or("sync", String::as_str);
    assert!(matches!(execution, "sync" | "mixed"));
    let mixed = execution == "mixed";
    let construction = args.get(8).map_or("provide", String::as_str);
    assert!(matches!(construction, "provide" | "construct" | "alternating"));
    assert!(construction == "provide" || shapes.split(',').all(|shape| matches!(shape, "fields100" | "fields500")));
    assert!(!shapes.split(',').any(|shape| matches!(shape, "fields100" | "fields500")) || (scope == "default" && !mixed));
    assert!(!mixed || shapes.split(',').all(|shape| matches!(shape, "chain100" | "flat500")));
    assert!(shapes
        .split(',')
        .all(|shape| matches!(shape, "chain100" | "flat500" | "edges500" | "edges2000" | "fields100" | "fields500")));
    assert!(only.is_none_or(|only| only
        .split(',')
        .all(|measurement| matches!(measurement, "clean" | "clean-app" | "body" | "topology" | "release"))));
    let target = output.join("target");
    let version = Command::new("rustc").arg("-Vv").output().unwrap();
    fs::write(output.join("toolchain.txt"), version.stdout).unwrap();
    let mut results = String::from("shape,measurement,seconds,bytes\n");
    for shape in shapes.split(',') {
        let dir = output.join(shape);
        fs::create_dir_all(dir.join("src")).unwrap();
        let features = if mixed { ", features = [\"async\"]" } else { "" };
        let runtime = if mixed {
            "tokio = { version = \"1\", features = [\"rt\"] }\n"
        } else {
            ""
        };
        fs::write(dir.join("Cargo.toml"), format!("[package]\nname = \"compile-bench-app\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[workspace]\n[dependencies]\nfroodi = {{ path = {:?}{features} }}\n{runtime}", repo.join("froodi"))).unwrap();
        let src = dir.join("src/main.rs");
        let original = source(shape, "none", scope, mixed, construction);
        fs::write(&src, &original).unwrap();
        run(cargo(&dir, &target).args(["build", "--offline"]), &dir.join("warm.log"));
        for measurement in ["clean", "clean-app", "body", "topology", "release"] {
            if measurement == "clean" && only.is_none() {
                continue;
            }
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
                if measurement == "clean" || measurement == "clean-app" || release {
                    let mut clean = cargo(&dir, &target);
                    clean.arg("clean");
                    if measurement != "clean" {
                        clean.args(["-p", "compile-bench-app"]);
                    }
                    if release {
                        clean.arg("--release");
                    }
                    run(&mut clean, &dir.join("clean.log"));
                } else {
                    fs::write(&src, source(shape, measurement, scope, mixed, construction)).unwrap();
                }
                samples.push(run(&mut build, &dir.join(format!("{measurement}-{sample}.log"))));
                let binary = target.join(if release { "release" } else { "debug" }).join("compile-bench-app");
                bytes = fs::metadata(&binary).unwrap().len();
                run(&mut Command::new(binary), &dir.join("run.log"));
            }
            let mut ordered = samples.clone();
            ordered.sort_by(f64::total_cmp);
            let seconds = ordered[ordered.len() / 2];
            println!("{shape}/{measurement}: {seconds:.3}s {bytes} bytes ({samples:?})");
            results.push_str(&format!("{shape},{measurement},{seconds:.6},{bytes}\n"));
            fs::write(output.join("results.csv"), &results).unwrap();
        }
    }
}
