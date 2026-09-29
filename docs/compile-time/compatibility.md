# Native runtime integration

The optional `froodi/compiled` feature connects typed linking to the original
sync/async containers. Original dynamic registries continue to work, including in
the same container. There is no forwarding to `froodi-compile-runtime`.

## Programming model

```rust
use froodi::{Container, instance, Inject, InstantiateErrorKind, DefaultScope::App};
use froodi::compiled::registry;

fn text(number: Inject<u32>) -> Result<String, InstantiateErrorKind> {
    Ok(number.0.to_string())
}
let container = Container::new(registry! {
    provide(App, instance(7u32)),
    provide(App, text),
});
assert_eq!(&*container.get::<String>().unwrap(), "7");
```

The macro import is opt-in: `froodi::registry!` / `async_registry!` still produce
native dynamic registries. Typed macro results have a different return type.
`Container::new` accepts either with `compiled` enabled. Calls to
`new_with_start_scope` infer both generics; an explicit scope turbofish under this
feature becomes `::<Scope, _>`. The original signature is unchanged without the
feature. The [executable example](../../froodi/examples/compiled.rs) covers
App settings/Database and Request Repository/Service with both injection modes.

| Area | Integrated behavior / boundary |
|---|---|
| Instantiators | Original trait and independently defined functions/captured closures; typed dependencies come from `Deps` |
| `instance` | Original helper; value ownership and cloning semantics unchanged |
| `Inject` / `InjectTransient` | Original get/transient lifecycle; no duplicate registration for modes |
| Scopes, Context, cache, close/drop | Original runtime implementation, including custom scopes and ancestor cache behavior |
| Finalizers | Original sync/async adapters and recording; replacements select matching finalizer and policy |
| Async | Same linker/topology; native async lifecycle and sync interoperability |
| Static ambiguity | Referenced duplicate static providers fail inference, including aliases |
| Runtime overrides | Native later-wins merge; no `.replacing()` requirement; final IDs remapped after composition |
| Runtime/Context boundary | Typed consumers declare `runtime::<T>()` / `context::<T>()`; ordinary dynamic registrations need no declarations |
| Custom resolvers | Typed parameters use `compiled::Resolver<T>`; dynamic parameters unchanged; arbitrary user lookups opaque |
| Returning fragments | Keep typed fragments within inference or erase using the sync/async `IntoRegistry` trait; erasure retains indexed edges but defers cycle checks |
| `froodi-auto` | Its existing dynamic registries can be opaque fragments; a typed auto frontend is not implemented |

Closed typed constructors retain static provider and sync/async checks. The const
cycle check covers at most 1,024 total nodes at code generation; it does not run
in `cargo check` or uninstantiated code. Open/erased/oversized registries retain
runtime graph validation. Missing opaque dependencies can be supplied by native
Context; absent values fail on resolution as in native Froodi. Scope/config
validation is runtime. See [architecture](architecture.md#bounded-static-cycle-validation).

## Verification and limits

Compatibility scenarios run against original, experimental and integrated paths.
Native regression suites cover edge order/repeats with resolver skipping, aligned
values, imports, Context, replacements before/after erasure, finalizer ownership,
captured instantiators and concurrent sync/async construction. The compile-stage
harness checks `check`, `build`, `test`, generic wrappers and oversized fallback.

The combined native/backend suite passes 420 tests. Native default/all-features
suites and final feature builds were also checked. Library Clippy passes with
warnings denied; linting all test targets still encounters existing acronym and
unused-generic test lints, which this integration does not change.

Focused integrated Miri runs pass with nightly 1.101.0 (`c1070d693`, 2026-09-28),
in local mode and for thread-safe adapters/async construction:

```sh
cargo +nightly miri test -p froodi --no-default-features --features compiled,async \
  --test compiled --test compiled_construction
cargo +nightly miri test -p froodi --features compiled,async \
  --test compiled --test compiled_construction -- --skip concurrent_cached_construction
```

Contended synchronous locking is **not fully Miri-verified**. Some schedules pass;
others reach `parking_lot_core 0.9.12`'s Linux futex call, where this Miri version
reports an incompatible C-variadic argument (`&Atomic<i32>` versus `*mut u32`).
The ordinary dynamic registry reproduces the same failure:

```sh
MIRIFLAGS=-Zmiri-preemption-rate=1 cargo +nightly miri test -p froodi \
  --features compiled,async --test compiled_construction native_concurrent_cached_construction
```

This dependency/Miri boundary remains unresolved; native synchronization was not
replaced to bypass it. Host concurrency tests pass for both paths. Adapter ownership,
remapping, aligned transient values and finalizers pass Miri in both configurations.

The native library checks with `--no-default-features --features compiled` and
`compiled,async` (alloc direction); these are host checks, not proof of support
on a particular bare-metal target. Native synchronization remains parking_lot /
Tokio; the experimental `lock-spin` backend is separately checked. Proc-macro
compiler dependencies stay on the host side. `compiled` requires the newer
experimental backend toolchain; the native crate's historical MSRV is not a
claim for this opt-in feature.

Remaining work: frontend stabilization/publishing of shared crates, clearer
ambiguity diagnostics, typed cross-crate fragment interfaces, and eventual
experimental runtime retirement. General concurrent close, cancellation and
shutdown questions belong to a separate Froodi lifecycle audit. The materialized
boundary review and Miri runs do not settle those questions.
