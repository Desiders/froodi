# Benchmarks

## Runtime

```sh
cargo bench -p froodi-compile --bench compare
```

The Criterion suite uses checked-in Rust fixtures in
[`benches/support/graphs.rs`](../../froodi-compile/benches/support/graphs.rs).
It compares Froodi, statically linked registrations (`static`) and runtime-linked
registrations (`indexed`). Both compile-time engine variants use the same indexed
execution backend.

Cases cover container construction, cached access, cold and transient 100-dependency
chains, a 16-wide graph, scope transitions, captured instantiators and a mixed
static/runtime boundary. Run with the default `thread_safe` feature so all engines
use the same `Arc` values.

For a change comparison, save a baseline before editing and compare afterward:

```sh
cargo bench -p froodi-compile --bench compare -- --save-baseline before
cargo bench -p froodi-compile --bench compare -- --baseline before
```

Record the toolchain and host, keep feature/profile settings identical, and avoid
competing workloads. Short samples are useful smoke checks but need confirmation
before attributing small timing changes to code.

## Build cost

The deep-chain and 500-registration fixtures are maintained directly in Rust:

```sh
cargo test -p froodi-compile --test scale --test registry_scale --no-run --timings
```

Cargo writes an HTML report under `target/cargo-timings`. Record whether artifacts
were already built. To measure a clean build without deleting existing artifacts,
use a fresh `CARGO_TARGET_DIR`. Compare clean and incremental builds separately;
provider-body edits and registry-shape edits exercise different type-checking work.

## Prior execution experiment

The removed `direct-edges` backend reduced measured cold-chain resolution by about
7% and transient-chain resolution by about 35% versus indexed dispatch. It also
made dependency depth drive recursive trait obligations: the 100-dependency debug
build took about 13.8 s versus 3.9 s for the table, and ordinary chains hit recursion
limits. These results motivate keeping one indexed backend. Revisit direct dispatch
only if profiling identifies indexed calls as a bottleneck; the experiment is not
an available feature.
