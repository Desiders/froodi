# Compiled frontend compatibility

The compiled frontend uses the original `froodi::Container` and async container.
Use `froodi::compiled_registry!` or, with `async`, `froodi::compiled_async_registry!`.
It is explicitly opt-in; enabling its feature never switches ordinary macros.

```rust
use froodi::{compiled_registry, instance, Container, Inject, InstantiateErrorKind, DefaultScope::App};

fn text(number: Inject<u32>) -> Result<String, InstantiateErrorKind> {
    Ok(number.0.to_string())
}
let container = Container::new(compiled_registry! {
    provide(App, instance(7u32)),
    provide(App, text),
});
assert_eq!(&*container.get::<String>().unwrap(), "7");
```

The [executable example](../../examples/sync_compiled_registration/src/main.rs), run with
`cargo run -p sync_compiled_registration`, uses the same `Config`, `Greeter` and `WelcomeHandler`
as [sync_registration](../../examples/sync_registration/src/main.rs), with the
`compiled_registry` import. Instantiators, captured closures, `instance`, injection modes,
cache policy, scopes, Context, finalizers and close/drop use native behavior.
Dynamic duplicates retain later-wins handling; referenced duplicate static
providers fail inference.

## Necessary API differences

- Typed macro results differ from dynamic `Registry`. `Container::new` accepts
  either with `compiled` enabled.
- Dynamic `new_with_start_scope::<Scope>` stays unchanged under feature unification.
  Typed registries use `new_compiled_with_start_scope::<Scope, _>`; inference also
  works. Hiding its linking witness in nested `impl Trait` is rejected by rustc
  (E0666). No proof types enter runtime execution.
- Compiled custom parameters use `froodi::RuntimeDependency<Custom>`, equivalent to a
  dynamic `Custom` parameter implementing the ordinary `DependencyResolver`.
  A blanket candidate also matches `Inject<T>`, making valid linking ambiguous
  and accepting missing providers through resolver fallback. The
  [candidate reproduction](../../froodi/tests/ui/compiled/support/resolver_candidates.rs)
  verifies this constraint. Runtime dependency parameters consume no indexed edge.
- Typed consumers declare `froodi::runtime::<T>()` / `froodi::context::<T>()` for
  opaque boundaries.
  Ordinary dynamic registries need no declarations. Native replacements require
  no static-only override marker. Erasing a fragment retains indexed execution
  but postpones cycle checks until effective composition.

## Validation and test organization

Missing/ambiguous static providers and sync-to-async dependencies fail checking.
Closed cycles fail only during instantiated build/test code generation within
1,024 typed declaration leaves, including implicit registrations. Open/erased/oversized graphs
retain runtime checks; scope/config values remain runtime. Arbitrary instantiator
and resolver lookups remain opaque. See [architecture](architecture.md).

The [compilation tests](../../froodi/tests/compile_fail.rs) use trybuild and `.stderr`
snapshots. Static-linking diagnostics use a fail-only suite (`cargo check`);
const cycles use a mixed pass/fail suite (`cargo build`). Run the whole cycle suite
when filtering: removing all pass fixtures with `trybuild=` switches it to checking.
[Ordinary validation tests](../../froodi/tests/validation.rs) cover valid composition
and oversized runtime fallback. Each large boundary graph is compiled once. The
500-independent-registration case shares the existing native scale fixture.
Unit tests under `src/compiled` cover lifecycle, indexed edges, local execution
and controlled construction scheduling. Downstream and compilation tests remain
external to verify public access and compiler behavior.

```sh
cargo test --workspace --all-features
cargo test -p froodi --no-default-features --features compiled,async
```

Refresh diagnostic snapshots after deliberate changes with
`TRYBUILD=overwrite cargo test -p froodi --features compiled,async --test compile_fail errors`.

The [downstream test](../../froodi/tests/downstream.rs) uses a reusable fixture in
`tests/fixtures/consumer` that renames Froodi to `di` and checks sync/async APIs.
Its test creates a minimal second dependency in the temporary project to verify
feature unification; that dependency exists only in the temporary project.
Disposable projects live outside the workspace. The same test can consume extracted package archives with
`FROODI_PACKAGE_DIR`; it rejects checkout references in that dependency graph.
There is no one-off packaging tool or patched release manifest.

## Distribution prerequisite

The backend is internal to Froodi. Registry parsing and auto-registration
attributes share `froodi-macros`, using `version = "1"` plus a workspace path.
The renamed macro package retains version `1.0.0`.

`cargo package -p froodi-macros --allow-dirty --all-features` passes. Normal
packaging of Froodi and froodi-auto fails with `no matching package named
froodi-macros found` in crates.io. Publishing `froodi-macros 1.0.0` is therefore
a prerequisite for their releases.

Both packages verify with local crates.io patches pointing to extracted archives,
and the downstream fixture passes against those archives without checkout
references. These checks do not establish registry availability. No package was
uploaded.

## Safety verification limitation

Focused suites pass on nightly 1.101.0 / Miri 0.1.0 (`5c543b0b8`, 2026-09-29):

```sh
cargo +nightly miri test -p froodi --no-default-features --features compiled,async \
  --lib compiled::tests
cargo +nightly miri test -p froodi --features compiled,async --lib compiled::tests::edges
cargo +nightly miri test -p froodi --features compiled,async --lib compiled::tests::construction \
  -- --skip concurrent_cached_construction
```

Contended synchronous locking is still **unresolved reported UB**, not an
unsupported operation. On `x86_64-unknown-linux-gnu`, with `parking_lot 0.12.5`,
`parking_lot_core 0.9.12`, `lock_api 0.4.14`, `libc 0.2.189`, Miri reports:

```text
Undefined Behavior: incorrect c-variadic argument type for `syscall(SYS_futex, ...)`:
expected argument #2 to have type `*mut u32` but got incompatible type
`&core::sync::atomic::Atomic<i32>`
parking_lot_core-0.9.12/src/thread_parker/linux.rs:112:13
```

The prior external-test reproduction reported the same diagnostic with `std::sync`
spelling. The unit-test reproduction above still reports UB; its cause is unresolved.

The shared controlled-construction reproduction is retained:

```sh
MIRIFLAGS=-Zmiri-preemption-rate=1 cargo +nightly miri test -p froodi \
  --features compiled --lib compiled::tests::construction::native_concurrent_cached_construction
```

It also reproduces with the compiled variant (`compiled::tests::construction::concurrent_cached_construction
-- --exact`). Ordinary reproduction identifies a shared path, not proof of
soundness or a tooling false positive. No race, deadlock or timeout was reported.
UB checks and synchronization were not changed to hide it. Native contention
regressions pass, but the Miri cause remains unresolved.

Host checks cover `no_std + alloc`, local async and thread-safe parking_lot/Tokio
configurations; they do not certify a bare-metal target. Parsing stays host-side.
The historical native MSRV is not a claim for the opt-in compiled feature, tested
on modern Rust. General concurrent close, cancellation and shutdown questions
remain a separate lifecycle concern.
