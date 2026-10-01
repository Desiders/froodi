# Compile-time and runtime benchmarks

The supported comparison is dynamic and compiled registrations in the original
Froodi containers. Both use native scopes, caches, synchronization and finalizers.

## Compile-time measurements

```sh
rustc --edition=2021 tools/compile_bench.rs -o /tmp/froodi-build-bench
/tmp/froodi-build-bench "$PWD" /tmp/froodi-build-measurements 3 both
```

Use a fresh output directory. The helper creates disposable downstream apps and
records sample logs, `results.csv` and `toolchain.txt`. Dependencies stay warm;
clean builds remove only app artifacts. Provider edits change the first
instantiator's result; shape edits add one unused `u64` registration. Release
executables are built and run. No compiler-internal stage hooks are required.

Actual shapes:

- **Chain 100:** 100 distinct value types, 99 immediate `Inject` edges. Each
  instantiator adds one to its predecessor's integer; the app requests the last.
- **Flat 500:** 500 distinct value types with independently defined, zero-parameter
  instantiators. There are **no dependency edges**. The app requests only the last
  value, but all registrations participate in linking and plan creation.

### Adapter comparison

The owner/pointer adapter and selected safe adapter used rustc 1.98.1
(`48a229cea`), LLVM 22.1.8, Ryzen 5 7500F and
`x86_64-unknown-linux-gnu`. Both used the same dependency lockfile
(SHA-256 `a1d77ed778c5dde30484164c7660ab550ecf2d31663f0a5e28618a5ad9438439`)
and checkout path, with `RUSTFLAGS` unset. Runtime comparisons use the unchanged
release profile; app compilation uses incremental dev builds with line-table
debug info. Practical slowdown
tolerances were set before selection: 5% runtime and 10% app compilation.
The compilation helper's stale generated macro import was corrected to the
existing public `froodi::compiled_registry` macro.

Three-sample medians, with warm dependencies and without CPU affinity:

| Compiled app measurement | Owner/pointer adapter | Safe adapter |
|---|---:|---:|
| Chain 100 clean | 1.734 s | 1.962 s |
| Chain 100 shape edit | 1.597 s | 1.755 s |
| Flat 500 clean | 7.392 s | 7.455 s |
| Flat 500 shape edit | 8.301 s | 7.555 s |

Chain clean ranges were 1.733–1.749 / 1.919–2.003 s; shape ranges were
1.544–2.033 / 1.724–1.859 s. Flat clean ranges were
7.132–7.407 / 7.312–7.487 s; shape ranges were
7.267–11.553 / 7.261–7.620 s. The 11.553 s sample was an outlier.
Chain clean increased 13.1%, exceeding the 10% tolerance; shape increased 9.9%.
This comparison is inconclusive for compiler regression. Current flat clean
builds are about 7.4 s; the difference from historical 3–4 s measurements remains
unresolved. No new provider-edit or release measurements were taken here.

### Historical consolidation measurements

Consolidation measurements used rustc 1.98.1 (`48a229cea`), LLVM 22.1.8,
Ryzen 5 7500F, `x86_64-unknown-linux-gnu`, default `std`/`thread_safe` plus
`compiled`. Dev builds enable incremental compilation and line-table debug info;
release debug info is disabled. Three-sample medians, without competing builds:

| Compiled app measurement | Chain 100 before / after | Flat 500 before / after |
|---|---:|---:|
| Clean build | 1.629 / 1.853 s | 3.576 / 3.617 s |
| Provider body edit | 0.249 / 0.293 s | 0.474 / 0.443 s |
| Registry-shape edit | 1.434 / 1.709 s | 3.548 / 3.511 s |
| Release build | 4.741 / 4.856 s | 8.131 / 7.818 s |
| Release executable | 1,934,104 / 1,932,584 B | 2,426,560 / 2,409,120 B |

After clean samples: chain 1.781–1.870 s, flat 3.570–3.858 s. Shape samples:
chain 1.653–4.406 s, flat 3.406–3.977 s. Chain body edits also included a
0.760 s outlier. Flat cost was about 3.5–3.6 s; these data do not establish
a small speedup. Chain medians increased, with substantial shape-edit variability.

Dynamic/compiled comparison at consolidation, using the same apps and warm-dependency policy:

| App / measurement | Dynamic | Compiled |
|---|---:|---:|
| Chain 100 clean | 1.287 s | 1.853 s |
| Chain 100 shape edit | 0.674 s | 1.709 s |
| Flat 500 clean | 1.876 s | 3.617 s |
| Flat 500 shape edit | 1.702 s | 3.511 s |

Dynamic clean ranges were 1.256–1.311 / 1.714–1.900 s; shape ranges were
0.566–0.856 / 1.496–2.044 s. Compiled timing includes its additional static
guarantees; dynamic timing does not enable that feature.

## Runtime measurements

`just bench-compiled` runs compiled variants of the existing initialization,
resolution and concurrent-resolution benchmarks using the same fixtures.
Each category also has a `bench-*-compiled` recipe. `just bench-compare-compiled`
and `just bench-compare-concurrent-compiled` compare compiled Froodi with the
other containers; `just bench-compare-registries` compares dynamic and compiled
Froodi, including the complete request lifecycle.

```sh
cargo bench -p froodi --features compiled,async --bench compiled_registry -- \
  --warm-up-time 0.5 --measurement-time 1.5 --sample-size 40 --noplot
```

The command uses the existing Criterion dependency and release profile, default
`std`/`thread_safe` plus `compiled,async`, and saves estimates under
`target/criterion`. It is a short suite run; the comparison below used longer
measurements on the toolchain, host and lockfile recorded above.

The shared [fixtures](../../froodi/benches/support/graphs.rs) contain 100 distinct
values: one chain retains predecessor `Arc`s using `Inject`; another owns
predecessors through `InjectTransient` and `Box`. Application work is limited to
retaining those values. The instantiator functions are shared between frontends.

Retained-container resolution excludes setup and container destruction. Startup,
scope entry and teardown are timed separately. The complete case creates App,
enters 32 Request scopes, performs first and pointer-identical cached gets plus
transient root construction in each, closes/drops each Request, then closes/drops
App. Its values all belong to Request. No custom finalizer work is timed.

### Adapter comparison

Both saved benchmark executables were built in the same checkout and run with
`taskset -c 2`, without competing workloads. Recorded Criterion settings:

| Filter | Warm-up | Measurement | Samples |
|---|---:|---:|---:|
| `registry_resolution` | 1 s | 8 s | 100 |
| `first_get_chain_100_retained` repeat | 2 s | 12 s | 120 |
| `native_lifecycle` | 1 s | 4 s | 80 |
| `async_instantiator` | 1 s | 5 s | 100 |

Apply the filter and settings to the command above and prefix it with
`taskset -c 2`. The async case uses a current-thread Tokio runtime and a
zero-dependency instantiator returning `u64`; each call requests a transient value.

Compiled medians with 95% median confidence intervals in brackets:

| Scenario | Owner/pointer adapter | Selected safe adapter |
|---|---:|---:|
| First App get, retained chain 100 | 17.20 [16.73–17.67] µs | 16.16 [15.81–16.56] µs |
| Entire transient chain 100 | 7.3347 [7.2886–7.4272] µs | 7.3349 [7.3095–7.3731] µs |
| Cached App get, control | 41.54 [41.31–42.09] ns | 38.17 [37.83–38.34] ns |
| App startup, Request chain | 43.03 [42.35–43.42] µs | 38.18 [38.11–38.33] µs |
| Enter Request | 76.44 [76.27–76.83] ns | 76.45 [76.08–77.13] ns |
| First Request get, chain 100 | 11.85 [11.79–11.92] µs | 11.49 [11.44–11.58] µs |
| Cached Request get, control | 40.90 [40.73–41.24] ns | 38.37 [37.80–38.71] ns |
| Transient root, warm injected dependencies | 52.15 [51.83–52.53] ns | 50.94 [50.66–51.28] ns |
| Close/drop populated Request | 1.54 [1.53–1.56] µs | 1.53 [1.52–1.56] µs |
| Close/drop App plan | 10.55 [9.17–11.41] µs | 8.52 [8.41–8.65] µs |
| Complete 32 requests | 515.00 [513.29–519.78] µs | 504.04 [499.01–511.12] µs |
| Async transient instantiation | 68.65 [68.26–69.05] ns | 61.57 [61.27–62.15] ns |

The first safe candidate increased deep transient time from 7.35 to 7.73 µs,
above the 5% tolerance. Generated code showed an extra wrapper call. Inlining
only `ErasedInstantiator::call` removed that cost; the corrected pair above and
the other compiled scenarios fit the runtime tolerance. The safe adapter is
selected without additional instantiator clones or type-based edge selection.

Dynamic controls and cached calls also shifted: dynamic cached App get changed
41.63 [41.31–41.85] to 38.40 [38.14–38.60] ns, and the dynamic complete scenario
changed 646.01 [638.94–655.76] to 608.05 [604.29–611.95] µs. These controls and
variation between repeats preclude attributing apparent speedups to the adapter.
The compiler tolerance remains unresolved.

### Historical consolidation measurements

Median estimates immediately before and after consolidation (40 samples,
0.5 s warm-up, 1.5 s requested measurement):

| Scenario | Dynamic before / after | Compiled before / after |
|---|---:|---:|
| App startup, Request chain | 14.73 / 17.87 µs | 37.84 / 45.41 µs |
| Enter Request | 80.57 / 79.46 ns | 79.48 / 81.73 ns |
| First Request get, chain 100 | 15.05 / 19.32 µs | 12.29 / 13.97 µs |
| Cached Request get | 29.47 / 23.24 ns | 28.72 / 22.36 ns |
| Transient root, warm injected dependencies | 60.58 / 53.90 ns | 47.38 / 54.81 ns |
| Close/drop populated Request | 1.63 / 1.80 µs | 1.57 / 1.97 µs |
| Close/drop App plan | 3.12 / 4.28 µs | 8.42 / 10.72 µs |
| Complete 32 requests | 535.82 / 724.93 µs | 483.96 / 591.71 µs |
| First App get, retained chain 100 | 16.18 / 24.50 µs | 14.49 / 22.63 µs |
| Entire transient chain 100 | 7.54 / 8.65 µs | 6.43 / 7.55 µs |

Startup, cached access, retained first access, deep transient construction and
the complete scenario were repeated. Complete-scenario after medians changed
from 758.89 / 611.37 µs to 724.93 / 591.71 µs. The latter 95% confidence intervals
were 709.89–743.13 / 582.30–599.06 µs, from 2,460 / 3,280 timed iterations.
Request-first-access intervals were 18.71–20.24 / 13.80–14.08 µs. App teardown
was noisier: 3.57–4.73 / 9.60–13.68 µs. These timings are not interchangeable
with cached or cold-resolution costs.

The saved pre-consolidation benchmark executable was also rerun: complete totals
were 526.93 / 516.32 µs. The new complete-case slowdown therefore persists beyond
one run. Consolidating fixtures changes nominal type identities, instantiator
placement and code layout; the native runtime still uses type-keyed caches and
maps. This comparison cannot isolate those effects from moving the backend.
It does **not** demonstrate unchanged runtime performance. Construction and
teardown differences need focused profiling before claiming a runtime improvement
from consolidation; no executor redesign was attempted here.

In that comparison, compiled first dependency-chain access was faster, but
startup and App-plan destruction cost more. Cached-access intervals overlap.
Static dependency adapters select by registration ID; outer `get`, native caches,
custom resolvers, dynamic adapters and startup remapping still use type keys.

## Historical design measurements

Ending path/Root propagation at linking reduced the old flat-500 build from about
12 s to about 3–4 s. Before native integration, isolated linking versus linking
plus const validation measured 0.549 / 0.594 s for chain 100 and 1.913 / 2.213 s
for flat 500. Those stage hooks and the experimental container are retired.

The removed direct-edge experiment saved about 7% cold-chain and 35% transient
time, but increased the debug chain build from about 3.9 s to 13.8 s and made
dependency depth drive trait recursion. Indexed execution remains the sole backend.
