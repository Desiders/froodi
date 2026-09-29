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

## Materialization experiment

Baseline: `db88e540`, with stage instrumentation. Measurements use rustc 1.98.1
(LLVM 22.1.8), AMD Ryzen 5 7500F (6 cores/12 threads), x86_64 Linux, default `std`/`thread_safe`, incremental compilation
enabled, dev `debug = "line-tables-only"`, and release optimization level 3.
Times are medians of three app builds; dependency compilation is excluded.
Release sizes include symbol tables but omit debug information.

### Stage breakdown before refactoring

Stages C and D independently extend B; these are cumulative timings, not additive
costs. A black-box consumer keeps each selected stage instantiated.

| Stage | What gets compiled | Chain 100 | Flat 500 |
|---|---|---:|---:|
| typed | Macro expansion and typed tree only | 0.18 s | 0.46 s |
| linking | A + linking | 0.53 s | 2.59 s |
| metadata | B + linked metadata | 1.32 s | 4.27 s |
| executors | B + executor collection | 2.28 s | 10.96 s |
| full | Full container construction and get | 2.67 s | 12.50 s |

The flat registry has no dependency depth, yet executor generation accounts for
much of its build cost. A separate rustc phase run of the full flat app spent
1.60 s type-checking, 4.70 s collecting monomorphizations and 2.35 s generating LLVM
IR. These phase timings overlap with other compiler work and should not be summed
as a complete wall-clock breakdown. Macro expansion was small relative to these
stages.

### Materialization without a lightweight index

This variant removes `Root` from execution and `Links` from linked registrations,
merges provider lookup/indexing, and materializes IDs in linking. Lookup still uses
the full instantiator-value tree.

| Measurement | Chain before | Chain materialized | Flat before | Flat materialized |
|---|---:|---:|---:|---:|
| clean-app | 2.67 s | 1.06 s | 12.50 s | 4.40 s |
| body | 0.27 s | 0.27 s | 0.95 s | 0.49 s |
| topology | 2.74 s | 1.07 s | 13.49 s | 4.49 s |
| release | 3.95 s | 2.40 s | 14.80 s | 7.91 s |

The flat case falls to 4.40 s, but linking alone still costs 2.61 s. This motivated
the separate lightweight provider-index experiment.

### Lightweight provider index

This experiment changed only provider lookup: `RegistryIndex::Index` contains
`Provider<Out>` / `AsyncProvider<Out>` leaves and preserves tree positions.
The table compares three-sample medians with the materialized value-tree variant.

| Clean app stage | Chain value tree | Chain index | Flat value tree | Flat index |
|---|---:|---:|---:|---:|
| linking | 0.66 s | 0.46 s | 2.61 s | 1.83 s |
| metadata | 0.72 s | 0.51 s | 3.03 s | 2.22 s |
| executors | 0.94 s | 0.74 s | 3.97 s | 3.25 s |
| full | 1.06 s | 0.86 s | 4.40 s | 3.45 s |

The projection overhead was outweighed by reduced proof types and specialization.
The index is retained. Execution compatibility was subsequently separated from
provider inference to restore specific sync-to-async diagnostics; the final
measurements below include that check, removal of unused value-tree lookup impls,
and the runtime-path adjustments described below.

### Final before/after

| Measurement | Chain before | Chain final | Flat before | Flat final |
|---|---:|---:|---:|---:|
| clean-app | 2.67 s | 1.32 s | 12.50 s | 3.63 s |
| body | 0.27 s | 0.23 s | 0.95 s | 0.52 s |
| topology | 2.74 s | 0.92 s | 13.49 s | 3.91 s |
| release | 3.95 s | 2.23 s | 14.80 s | 7.50 s |
| Release executable | 2.56 MB | 1.09 MB | 11.77 MB | 1.96 MB |

| Final stage | Chain 100 | Flat 500 |
|---|---:|---:|
| typed | 0.18 s | 0.49 s |
| linking | 0.59 s | 2.08 s |
| metadata | 0.65 s | 2.27 s |
| executors | 0.88 s | 3.17 s |
| full | 1.32 s | 3.63 s |

Linking now includes ID materialization and execution validation; metadata generation
moves the stored request vectors into the IR. Flat-500 clean builds improved by 71%, and topology
edits by 71%, reaching the requested 3–4 s range without fragment interface erasure.
The final flat-app rustc phase run spent 0.52 s type-checking, 1.41 s collecting
monomorphizations and 0.73 s generating LLVM IR (3.82 s total). Macro expansion
remained about 0.03 s.

The last run had visible wall-clock variation: chain clean-build samples ranged
from 1.18–1.42 s, and flat topology edits from 3.70–6.21 s. Tables report the medians
without discarding samples; treat small differences between variants as noise.

## Runtime comparisons

```sh
cargo bench -p froodi-compile --bench compare -- --save-baseline before
# Apply the change, then:
cargo bench -p froodi-compile --bench compare -- --baseline before
```

The checked-in Rust fixtures in
[graphs.rs](../../froodi-compile/benches/support/graphs.rs) compare Froodi, statically
linked registrations (`static`) and runtime-linked registrations (`indexed`).
Both compile-time engine variants use the same indexed execution backend. Cases
cover construction, cached and first access, deep transient resolution, scopes,
captured instantiators and runtime boundaries. Default `thread_safe` features keep
all engines on `Arc`.

### Runtime-path adjustments

The first materialized version retained copies of request vectors and looked up
compiled edges again inside each executor. Its static cold-chain and transient
benchmarks regressed. The final implementation moves request vectors into the IR
and passes the selected compiled edge slice directly to construction. Static
parameter resolution uses the proven edge count to avoid a repeated iterator
bounds check; that unsafe precondition is documented beside the executor. Runtime
registrations retain checked resolution. None of these changes restores root/path
types to execution or adds a type lookup to static edges.

### Before/after runtime results

Criterion, 1 s warm-up, 3 s measurement, 60 samples per case; median point estimates
from the final full run. These runs used the same fixtures and default features.

| Static registration scenario | Before | Final | Change |
|---|---:|---:|---:|
| container_new_chain_100 | 23.53 µs | 24.12 µs | 2.5% |
| get_cached | 22.13 ns | 22.15 ns | 0.1% |
| first_get_chain_100 | 7.55 µs | 7.83 µs | 3.6% |
| enter_and_resolve_request_chain_100 | 5.91 µs | 6.16 µs | 4.1% |
| enter_build_scope_transition | 79.72 ns | 88.57 ns | 11.1% |
| get_transient_chain_100 | 3.93 µs | 4.08 µs | 3.8% |
| first_get_wide_16 | 433.13 ns | 466.65 ns | 7.7% |
| enter_and_resolve_captured_closure | 126.84 ns | 136.56 ns | 7.7% |
| first_get_mixed_boundary | 278.74 ns | 301.92 ns | 8.3% |

Cached access is unchanged within measurement noise. Static construction now loads
numeric targets from compiled metadata instead of embedding them as constants in
root-specialized functions. This adds runtime work, but the measurements alone do
not attribute every difference to it, particularly the scope-only case. The
compile-time improvement is therefore not a claim of identical runtime timings.
The larger first-version regressions were removed; the remaining measured trade-off
is recorded above, including scope transitions and mixed-boundary cases. An earlier
focused cold-chain confirmation measured 7.44 µs, so small run-to-run changes should
not be overinterpreted. Static edges still avoid type lookup and remain faster than
Froodi on these deep-chain fixtures: Froodi's final cold-chain and transient medians
were 22.00 µs and 7.07 µs, respectively.

## Prior execution experiment

The removed `direct-edges` backend reduced measured cold-chain resolution by about
7% and transient-chain resolution by about 35% versus indexed dispatch. It also
made dependency depth drive recursive trait obligations: the 100-dependency debug
build took about 13.8 s versus 3.9 s for the table, and ordinary chains hit recursion
limits. These results motivate keeping one indexed backend. Revisit direct dispatch
only if profiling identifies indexed calls as a bottleneck; the experiment is not
an available feature.
