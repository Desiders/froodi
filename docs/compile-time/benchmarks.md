# Compile-time and runtime benchmarks

## Reproduce compile-time measurements

The Rust helper creates isolated app crates with 100-dependency chains and 500
independent registrations, matching the scale-test shapes. It cleans only the app
between clean-build samples; dependency artifacts stay warm. It measures provider
body edits, topology edits (one extra unused registration), release builds and
release executable sizes. Full-engine binaries are also executed after building.

```sh
rustc --edition=2021 tools/compile_bench.rs -o /tmp/compile-bench
/tmp/compile-bench "$PWD" /tmp/froodi-compile-measurements 3 all
```

Populate the dependency cache first with `cargo build -p froodi-compile`; the helper
uses Cargo offline. Use a fresh output directory per revision. `stages` measures only clean app builds;
`full` measures only the full engine, including edits and release builds. Reports
include `results.csv`, individual Cargo logs and `time-passes.log` for each stage.
The helper enables the internal `compile-bench` runtime feature for stage hooks.

The phase logs use `RUSTC_BOOTSTRAP=1 cargo rustc -- -Ztime-passes` for measurement
only. Normal builds use stable Cargo settings. Record the toolchain and host, and
run without competing builds or benchmarks.

## Native Froodi integration

Baseline `6e6ab97`; rustc 1.98.1 / LLVM 22.1.8, Ryzen 5 7500F, x86_64 Linux.
Dependencies were warm, dev incremental compilation enabled, `debug =
"line-tables-only"`, default `std`/`thread_safe`. No other task builds ran during
measurements. Host scheduling still produced outliers; clean builds were repeated
with five samples. Shape edits use three-sample medians. Do not read small timing
differences as precise speedups.

| App / measurement | Native before | Experimental before | Integrated final |
|---|---:|---:|---:|
| Chain 100 clean | 1.28 s | 1.10 s | 1.63 s |
| Chain 100 shape edit | 0.47 s | 1.08 s | 2.02 s |
| Flat 500 clean | 1.70 s | 4.11 s | 3.69 s |
| Flat 500 shape edit | 1.37 s | 4.59 s | 3.86 s |

Final flat-500 clean samples were 3.48–5.71 s; shape-edit samples were
3.43–6.18 s. Tables report medians; host variability limits precision.

Reproduce against an untouched baseline checkout and the integration checkout:

```sh
rustc --edition=2021 tools/compile_bench.rs -o /tmp/compile-bench
/tmp/compile-bench "$baseline_repo" /tmp/native-before 3 full froodi
/tmp/compile-bench "$baseline_repo" /tmp/experimental-before 3 full experimental
/tmp/compile-bench "$PWD" /tmp/native-integrated 3 full integrated
# Optional last argument selects one measurement, e.g. repeat noisy clean samples:
/tmp/compile-bench "$PWD" /tmp/native-confirm 5 full integrated clean-app
```

### Runtime

Criterion median point estimates, 40 samples, 0.5 s warm-up and 1.5 s measurement.
The native before column uses the untouched baseline. The after columns use the
integrated revision and shared graph types, with the `compiled` feature enabled.
The retained-container case was measured separately after adding its instrumentation.

| Scenario | Native before | Native dynamic after | Experimental after | Integrated after |
|---|---:|---:|---:|---:|
| Container creation, chain 100 | 15.01 µs | 16.22 µs | 24.21 µs | 46.14 µs |
| Cached get | 22.72 ns | 25.50 ns | 23.74 ns | 26.36 ns |
| First get, chain 100, including teardown | 21.80 µs | 22.90 µs | 7.92 µs | 25.93 µs |
| First get, chain 100, retained container | 18.88 µs | 17.56 µs | 4.58 µs | 15.03 µs |
| Enter/resolve Request chain 100 | 15.23 µs | 15.31 µs | 6.15 µs | 12.63 µs |
| Scope transition | 131.02 ns | 124.55 ns | 103.64 ns | 124.80 ns |
| Transient chain 100 | 7.03 µs | 7.82 µs | 4.10 µs | 6.79 µs |
| First get, wide 16, including teardown | 498.30 ns | 595.98 ns | 456.55 ns | 823.22 ns |
| Enter/resolve captured closure | 178.51 ns | 194.00 ns | 139.01 ns | 182.05 ns |

The existing `first_get` cases consume their container inside the timed closure.
The added `first_get_chain_100_retained` uses `iter_batched_ref` to exclude input
and plan destruction. It was added to baseline benchmark instrumentation too;
baseline runtime code was unchanged. This separates construction from the larger
integrated plan's teardown cost. The experimental retained-container baseline
was 4.45 µs.

Static parameters now select providers by ID, including static edges into runtime
registrations. Public get, native caches/locks, custom resolvers and dependencies
inside ordinary dynamic adapters still do type-based work. Startup remapping is
also type-based. The integration improves some dependency-heavy cases but does
**not** inherit the experimental executor's overall speed: native lifecycle work
remains, and construction/teardown costs are higher. These are remaining tuning
costs before enabling the frontend by default, not evidence of a fallback to
provider lookup on static edges.

```sh
cargo bench -p froodi-compile --bench compare -- '(froodi|static|integrated)$' \
  --warm-up-time 0.5 --measurement-time 1.5 --sample-size 40 --noplot
```

## Static-validation cost

The experimental stage fixture isolates linking from mandatory const validation
without creating a container or executors. On rustc 1.98.1 / Ryzen 5 7500F, with
warm dependencies and three-sample medians:

| Stage | Chain 100 | Flat 500 |
|---|---:|---:|
| Linking | 0.549 s | 1.913 s |
| Linking + const validation | 0.594 s | 2.213 s |
| Additional time | 0.045 s | 0.300 s |

These measurements predate native integration; they isolate the shared validation
mechanism, not total integrated build cost. Both pairs produced identical executable
sizes. Use the helper's `stages` mode to repeat this comparison when changing the
validation limit or algorithm.

## Runtime fixtures

The checked-in [graphs.rs](../../froodi-compile/benches/support/graphs.rs) compares
native Froodi (`froodi`), experimental static registrations (`static`), experimental
runtime registrations (`indexed`) and the integrated native container
(`integrated`). Both experimental variants use indexed execution. Cases cover
construction, cached and first access, deep transient resolution, scopes, captured
instantiators and runtime boundaries. Default `thread_safe` features keep all
engines on `Arc`.

To compare a future change:

```sh
cargo bench -p froodi-compile --bench compare -- --save-baseline before
# Apply the change, then:
cargo bench -p froodi-compile --bench compare -- --baseline before
```

## Prior execution experiment

The removed `direct-edges` backend reduced measured cold-chain resolution by about
7% and transient-chain resolution by about 35% versus indexed dispatch. It also
made dependency depth drive recursive trait obligations: the 100-dependency debug
build took about 13.8 s versus 3.9 s for the table, and ordinary chains hit recursion
limits. These results motivate keeping one indexed backend. Revisit direct dispatch
only if profiling identifies indexed calls as a bottleneck; the experiment is not
an available feature.
