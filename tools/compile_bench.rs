//! rustc --edition=2021 tools/compile_bench.rs -o /tmp/froodi-build-bench
//! /tmp/froodi-build-bench <repo> <fresh-output> [samples=3] [dynamic|compiled|both] [measurement]
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
    command
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_INCREMENTAL", "1")
        .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
        .env("CARGO_PROFILE_RELEASE_DEBUG", "false");
    command
}

fn source(shape: &str, edit: &str, engine: &str) -> String {
    let count = if shape == "chain100" { 100 } else { 500 };
    let registry = if engine == "compiled" {
        "froodi::compiled_registry"
    } else {
        "froodi::registry"
    };
    let mut text = format!("#![allow(unused_imports, dead_code)]\nuse {registry};\nuse froodi::{{Container, Inject, InstantiateErrorKind, DefaultScope::App}};\n");
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
    text.push_str(&format!(
        "}}; let c = Container::new(registry); std::hint::black_box(c.get::<T{}>().unwrap().0); }}\n",
        count - 1
    ));
    text
}

fn main() {
    let args: Vec<_> = env::args().collect();
    assert!(
        args.len() >= 3,
        "usage: compile-bench <repo> <fresh-output> [samples=3] [dynamic|compiled|both] [measurement]"
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
    assert!(matches!(only, None | Some("clean-app" | "body" | "topology" | "release")));
    let target = output.join("target");
    let version = Command::new("rustc").arg("-Vv").output().unwrap();
    fs::write(output.join("toolchain.txt"), version.stdout).unwrap();
    let mut results = String::from("engine,shape,measurement,seconds,bytes\n");
    for engine in ["dynamic", "compiled"] {
        if selected != "both" && selected != engine {
            continue;
        }
        for shape in ["chain100", "flat500"] {
            let dir = output.join(format!("{engine}-{shape}"));
            fs::create_dir_all(dir.join("src")).unwrap();
            let features = if engine == "compiled" { "[\"compiled\"]" } else { "[]" };
            fs::write(dir.join("Cargo.toml"), format!("[package]\nname = \"compile-bench-app\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[workspace]\n[dependencies]\nfroodi = {{ path = {:?}, features = {features} }}\n", repo.join("froodi"))).unwrap();
            let src = dir.join("src/main.rs");
            let original = source(shape, "none", engine);
            fs::write(&src, &original).unwrap();
            run(cargo(&dir, &target).args(["build", "--offline"]), &dir.join("warm.log"));
            for measurement in ["clean-app", "body", "topology", "release"] {
                if only.is_some_and(|only| only != measurement) {
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
                        fs::write(&src, source(shape, measurement, engine)).unwrap();
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
