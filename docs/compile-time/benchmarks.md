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

Consolidation measurements use rustc 1.98.1 (`48a229cea`), LLVM 22.1.8,
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
0.760 s outlier. Flat cost remains about 3.5–3.6 s; these data do not establish
a small speedup. Chain medians increased, with substantial shape-edit variability.

Current dynamic/compiled comparison, using the same apps and warm-dependency policy:

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

```sh
cargo bench -p froodi --features compiled,async --bench compiled_registry -- \
  --warm-up-time 0.5 --measurement-time 1.5 --sample-size 40 --noplot
```

The existing Criterion dependency and release profile are used. The toolchain and
host match the compilation measurements; features are default `std`/`thread_safe`
plus `compiled,async`. Each case has 40 samples, 0.5 s warm-up and at least 1.5 s
requested measurement. Criterion saves iterations and confidence intervals under
`target/criterion`. No competing builds ran during these measurements.

The shared [fixtures](../../froodi/benches/support/graphs.rs) contain 100 distinct
values: one chain retains predecessor `Arc`s using `Inject`; another owns
predecessors through `InjectTransient` and `Box`. Application work is limited to
retaining those values. The instantiator functions are shared between frontends.

Retained-container resolution excludes setup and container destruction. Startup,
scope entry and teardown are timed separately. The complete case creates App,
enters 32 Request scopes, performs first and pointer-identical cached gets plus
transient root construction in each, closes/drops each Request, then closes/drops
App. Its values all belong to Request. No custom finalizer work is timed.

Median estimates immediately before and after consolidation:

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

Within the current suite, compiled first dependency-chain access is faster, but
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
