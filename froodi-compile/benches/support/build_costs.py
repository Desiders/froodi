#!/usr/bin/env python3
"""Measures build costs of the compile-time engine against current Froodi (issue #64).

Generates one small binary crate per (graph shape, engine) variant outside the workspace, builds
each one with its own target and build directories, and prints Markdown tables of:

- clean debug and clean release builds, in two meanings:
  full: the whole crate graph from an empty target directory (crates.io dependencies, the
  workspace engine crates and the application crate);
  app: only the application crate, after `cargo clean -p <app>` on a fully built target;
- incremental debug rebuilds after a provider body edit, a provider signature edit and two
  registry topology edits (remove an unused registration, add a new one);
- generated code size: `-Zunpretty=expanded` output of the application crate (nightly) and the
  LLVM IR of its release build;
- release binary size, unstripped and after `strip`.

Every timing is the wall-clock time of one `cargo build` process; the tables show the median of
`--reps` repetitions. Usage: python3 build_costs.py [--out DIR] [--reps N] [--only NAME ...]
"""

import argparse
import json
import os
import pathlib
import shutil
import statistics
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parents[3]
CHAIN = 100
FLAT = 500
MID = CHAIN // 2

ENGINES = {
    # name: (crate import path, dependency line, extra crate attributes)
    "froodi": ("froodi", f'froodi = {{ path = "{ROOT / "froodi"}" }}', ""),
    "compile": ("froodi_compile", f'froodi-compile = {{ path = "{ROOT / "froodi-compile"}" }}', ""),
    "direct": (
        "froodi_compile",
        f'froodi-compile = {{ path = "{ROOT / "froodi-compile"}", features = ["direct-edges"] }}',
        '#![recursion_limit = "512"]\n',
    ),
}
SHAPES = ("chain", "flat")

# The edits applied to the baseline source; each one is measured and then reverted.
EDITS = ("body", "signature", "topology_remove", "topology_add")


def source(shape, engine, edit=None):
    """The application's main.rs; `edit` names one of EDITS or None for the baseline."""
    crate, _, attrs = ENGINES[engine]
    literal = 2 if edit == "body" else 1
    extra_param = edit == "signature"
    out = [
        attrs + "#![allow(dead_code, unused_imports, clippy::all)]",
        "use std::sync::Arc;",
        f"use {crate}::{{registry, Container, DefaultScope::*, Inject, InjectTransient, InstantiateErrorKind}};",
        "pub struct Unused;",
        "pub struct Added;",
        "#[derive(Clone)]",
        "pub struct Extra;",
        "fn unused() -> Result<Unused, InstantiateErrorKind> { Ok(Unused) }",
        "fn extra() -> Result<Extra, InstantiateErrorKind> { Ok(Extra) }",
    ]
    if edit == "topology_add":
        out.append("fn added() -> Result<Added, InstantiateErrorKind> { Ok(Added) }")
    # The edited provider: `s{MID}` in the chain, `last` (the resolved element) in the flat registry.
    edited_param = "InjectTransient(_x): InjectTransient<Extra>" if extra_param else ""
    if shape == "chain":
        out.append("pub struct S0;")
        for i in range(1, CHAIN):
            # The previous link is stored erased: a 100-deep `Arc<Arc<...>>` type overflows drop-check.
            out.append(f"pub struct S{i}(pub Arc<dyn Send + Sync>);")
        out.append("fn s0() -> Result<S0, InstantiateErrorKind> { Ok(S0) }")
        for i in range(1, CHAIN):
            if i == MID:
                params = ", ".join(p for p in [f"Inject(p): Inject<S{i - 1}>", edited_param] if p)
                out.append(
                    f"fn s{i}({params}) -> Result<S{i}, InstantiateErrorKind> "
                    f"{{ std::hint::black_box({literal}u32); Ok(S{i}(p)) }}"
                )
            else:
                out.append(f"fn s{i}(Inject(p): Inject<S{i - 1}>) -> Result<S{i}, InstantiateErrorKind> {{ Ok(S{i}(p)) }}")
        provides = [f"provide(s{i})" for i in range(CHAIN)]
        target = f"S{CHAIN - 1}"
    else:
        out.append("pub struct T<const N: usize>;")
        out.append("fn inst<const N: usize>() -> Result<T<N>, InstantiateErrorKind> { Ok(T::<N>) }")
        out.append(
            f"fn last({edited_param}) -> Result<T<{FLAT - 1}>, InstantiateErrorKind> "
            f"{{ std::hint::black_box({literal}u32); Ok(T::<{FLAT - 1}>) }}"
        )
        provides = [f"provide(inst::<{i}>)" for i in range(FLAT - 1)] + ["provide(last)"]
        target = f"T<{FLAT - 1}>"
    if extra_param:
        provides.append("provide(extra)")
    if edit != "topology_remove":
        provides.append("provide(unused)")
    if edit == "topology_add":
        provides.append("provide(added)")
    body = ",\n            ".join(provides)
    out.append(
        f"""
fn main() {{
    let container = Container::new(registry! {{
        scope(App) [
            {body}
        ]
    }});
    let value = container.get::<{target}>().unwrap();
    std::hint::black_box(&value);
}}"""
    )
    return "\n".join(out) + "\n"


class Variant:
    def __init__(self, out, shape, engine):
        self.shape, self.engine = shape, engine
        self.name = f"bc_{shape}_{engine}"
        self.dir = out / self.name
        self.main = self.dir / "src" / "main.rs"

    def write(self, edit=None):
        self.main.write_text(source(self.shape, self.engine, edit))

    def setup(self):
        (self.dir / "src").mkdir(parents=True, exist_ok=True)
        dep = ENGINES[self.engine][1]
        (self.dir / "Cargo.toml").write_text(
            f'[package]\nname = "{self.name}"\nversion = "0.0.0"\nedition = "2021"\npublish = false\n\n'
            f"[dependencies]\n{dep}\n\n[workspace]\n"
        )
        # The workspace lock pins the same dependency versions the workspace builds with.
        shutil.copy(ROOT / "Cargo.lock", self.dir / "Cargo.lock")
        self.write()

    def env(self, kind="main"):
        env = dict(os.environ)
        # Own target and build directories: the user's global `build.build-dir` would otherwise
        # share one pruned directory under /tmp/cargo-build.
        env["CARGO_TARGET_DIR"] = str(self.dir / f"target-{kind}")
        env["CARGO_BUILD_BUILD_DIR"] = str(self.dir / f"build-{kind}")
        env.pop("RUSTC_WRAPPER", None)
        return env

    def dirs(self, kind="main"):
        return [self.dir / f"target-{kind}", self.dir / f"build-{kind}"]

    def cargo(self, args, kind="main", toolchain=None, stdout=None):
        # `--offline` goes before a `--` that starts rustc arguments.
        split = args.index("--") if "--" in args else len(args)
        cmd = ["cargo"] + ([f"+{toolchain}"] if toolchain else []) + args[:split] + ["--offline"] + args[split:]
        start = time.perf_counter()
        proc = subprocess.run(
            cmd, cwd=self.dir, env=self.env(kind), stdout=stdout or subprocess.PIPE, stderr=subprocess.PIPE, text=True
        )
        elapsed = time.perf_counter() - start
        with open(self.dir / "cargo.log", "a") as log:
            log.write(f"$ {' '.join(cmd)}  [{elapsed:.3f}s, exit {proc.returncode}]\n{proc.stderr}\n")
        if proc.returncode != 0:
            lines = [l for l in proc.stderr.splitlines() if l.strip()]
            first = next((i for i, l in enumerate(lines) if l.startswith("error")), 0)
            raise BuildError("\n".join(lines[first:first + 12]))
        return elapsed, proc


class BuildError(Exception):
    pass


def wipe(paths):
    for p in paths:
        shutil.rmtree(p, ignore_errors=True)


def measure(v, reps, log):
    res = {}

    def rec(key, values):
        res[key] = statistics.median(values)
        res[key + "_all"] = values
        log(f"  {key}: median {res[key]:.2f}s of {[round(x, 2) for x in values]}")

    for profile, flags in (("debug", []), ("release", ["--release"])):
        full = []
        for _ in range(reps):
            wipe(v.dirs())
            full.append(v.cargo(["build"] + flags)[0])
        rec(f"clean_full_{profile}", full)
        app = []
        for _ in range(reps):
            v.cargo(["clean", "-p", v.name] + flags)
            # `clean -p` keeps nothing of the package, incremental state included; checked below.
            app.append(v.cargo(["build"] + flags)[0])
        rec(f"clean_app_{profile}", app)

    # Incremental debug rebuilds from a built baseline.
    v.write()
    v.cargo(["build"])
    for edit in EDITS:
        times = []
        for _ in range(reps):
            v.write(edit)
            times.append(v.cargo(["build"])[0])
            v.write()
            v.cargo(["build"])
        rec(f"incr_{edit}", times)

    # Binary size of the release build (built above; rebuild is a no-op).
    v.cargo(["build", "--release"])
    binary = v.dir / "target-main" / "release" / v.name
    res["bin_bytes"] = binary.stat().st_size
    stripped = v.dir / f"{v.name}.stripped"
    subprocess.run(["strip", "-o", str(stripped), str(binary)], check=True)
    res["bin_stripped_bytes"] = stripped.stat().st_size

    # Expanded source of the application crate (nightly, check profile, own target dir).
    expanded = v.dir / "expanded.rs"
    with open(expanded, "w") as f:
        v.cargo(["rustc", "--profile=check", "--bin", v.name, "--", "-Zunpretty=expanded"],
                kind="nightly", toolchain="nightly", stdout=f)
    text = expanded.read_bytes()
    res["expanded_bytes"] = len(text)
    res["expanded_lines"] = text.count(b"\n")

    # LLVM IR of the application crate, release profile, one codegen unit so rustc writes one file.
    ir_dir = v.dir / "target-ir"
    wipe(v.dirs("ir"))
    v.cargo(["rustc", "--release", "--bin", v.name, "--", "--emit=llvm-ir", "-Ccodegen-units=1"], kind="ir")
    lls = [p for p in (v.dir / "build-ir").rglob("*.ll")] + [p for p in ir_dir.rglob("*.ll")]
    lls = [p for p in lls if p.name.startswith(v.name)]
    if lls:
        ll = max(lls, key=lambda p: p.stat().st_mtime)
        data = ll.read_bytes()
        res["ir_lines"] = data.count(b"\n")
        res["ir_bytes"] = len(data)
    return res


def fmt_s(x):
    return "-" if x is None else f"{x:.2f}"


def fmt_n(x):
    return "-" if x is None else f"{x:,}"


def table(results, variants):
    rows = []
    cols = [f"{v.shape} / {v.engine}" for v in variants]
    rows.append("| measurement | " + " | ".join(cols) + " |")
    rows.append("|---|" + "---:|" * len(cols))
    spec = [
        ("clean debug, full graph (s)", "clean_full_debug", fmt_s),
        ("clean debug, app crate only (s)", "clean_app_debug", fmt_s),
        ("clean release, full graph (s)", "clean_full_release", fmt_s),
        ("clean release, app crate only (s)", "clean_app_release", fmt_s),
        ("incremental: provider body edit (s)", "incr_body", fmt_s),
        ("incremental: provider signature edit (s)", "incr_signature", fmt_s),
        ("incremental: remove unused registration (s)", "incr_topology_remove", fmt_s),
        ("incremental: add registration (s)", "incr_topology_add", fmt_s),
        ("expanded source, lines", "expanded_lines", fmt_n),
        ("expanded source, bytes", "expanded_bytes", fmt_n),
        ("release LLVM IR (1 CGU), lines", "ir_lines", fmt_n),
        ("release LLVM IR (1 CGU), bytes", "ir_bytes", fmt_n),
        ("release binary, bytes", "bin_bytes", fmt_n),
        ("release binary stripped, bytes", "bin_stripped_bytes", fmt_n),
    ]
    for label, key, f in spec:
        cells = []
        for v in variants:
            r = results.get(v.name, {})
            cells.append("build failed" if "error" in r else f(r.get(key)))
        rows.append(f"| {label} | " + " | ".join(cells) + " |")
    return "\n".join(rows)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(pathlib.Path.cwd() / "build-costs"))
    ap.add_argument("--reps", type=int, default=3)
    ap.add_argument("--only", nargs="*", help="variant names, e.g. bc_chain_froodi")
    ap.add_argument("--no-direct", action="store_true", help="skip the direct-edges variants")
    args = ap.parse_args()
    out = pathlib.Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    logf = open(out / "run.log", "a")

    def log(msg):
        print(msg, file=sys.stderr, flush=True)
        logf.write(msg + "\n")
        logf.flush()

    variants = [
        Variant(out, s, e) for s in SHAPES for e in ENGINES if not (args.no_direct and e == "direct")
    ]
    if args.only:
        variants = [v for v in variants if v.name in args.only]
    log(f"# rustc: {subprocess.run(['rustc', '-V'], capture_output=True, text=True).stdout.strip()}")
    log(f"# nproc: {os.cpu_count()}, reps: {args.reps}, RUSTFLAGS: {os.environ.get('RUSTFLAGS', '')!r}")
    results = {}
    for v in variants:
        log(f"== {v.name}")
        v.setup()
        (v.dir / "cargo.log").unlink(missing_ok=True)
        try:
            # Warm-up: fetch nothing (offline), trim the lock file, prove the variant builds.
            v.cargo(["build"])
            results[v.name] = measure(v, args.reps, log)
        except BuildError as e:
            log(f"  build failed:\n{e}")
            results[v.name] = {"error": str(e)}
        (out / "results.json").write_text(json.dumps(results, indent=1))
    print(table(results, variants))
    for v in variants:
        if "error" in results[v.name]:
            print(f"\n{v.name} failed:\n```text\n{results[v.name]['error']}\n```")


if __name__ == "__main__":
    main()
