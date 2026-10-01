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
  either with `compiled` enabled. Use `.into_registry()` to return a native
  `froodi::Registry` from a function without naming the typed tree.
  Async compositions use `.into_async_registry()` and return
  `froodi::async_impl::RegistryWithSync`. Conversion preserves linking checks
  and indexed edges; final construction validates the effective graph after
  any later replacements.
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

Registration expressions are evaluated once in source order, including
interleaved extensions, scope expressions, config and finalizer options.
Tree balancing does not determine evaluation order.

Native fragments override typed registrations regardless of their position in
`extend(...)`; later native fragments override earlier native fragments. Typed
duplicates remain subject to static ambiguity checks. Restoring source-order
replacement across both kinds would require changes to collection and assembly;
expression evaluation follows source order independently.

## Validation and test organization

Missing/ambiguous static providers and sync-to-async dependencies fail checking.
Closed cycles fail only during instantiated build/test code generation within
1,024 typed declaration leaves, including implicit registrations. Open/erased/oversized graphs
retain runtime checks; scope/config values remain runtime. Arbitrary instantiator
and resolver lookups remain opaque. See [architecture](architecture.md).
Mixed sync/async compositions with compiled executors validate async dependencies
against the selected provider: async first, then sync. Purely native compositions
retain their existing validation behavior.

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

The contended parking_lot report is localized to a **dependency FFI argument
mismatch under Miri's C-variadic contract**. Standalone parking_lot contention and
direct syscall probes reproduce it on `x86_64-unknown-linux-gnu`, with
`parking_lot 0.12.5`, `parking_lot_core 0.9.12`, `lock_api 0.4.14` and
`libc 0.2.189`. Both pinned toolchains report UB:

- `nightly-2026-09-29`: rustc/Miri `c1070d693`, compiler date 2026-09-28.
- `nightly-2026-09-30`: rustc/Miri `5c543b0b8`, compiler date 2026-09-29.

```text
Undefined Behavior: incorrect c-variadic argument type for `syscall(SYS_futex, ...)`:
expected argument #2 to have type `*mut u32` but got incompatible type
`&core::sync::atomic::Atomic<i32>`
parking_lot_core-0.9.12/src/thread_parker/linux.rs:112:13
```

Passing `&AtomicI32` or `*const AtomicI32` fails for both WAIT and WAKE; passing
`word.as_ptr().cast::<u32>()` passes. WAIT deliberately requests 1 from a word
containing 0: native and accepted Miri calls return `-1`, errno `EAGAIN` (11).
This is reported UB, not an unsupported operation, race, deadlock or timeout.

An isolated copy with the two pointer changes proposed in
[parking_lot PR #539](https://github.com/Amanieu/parking_lot/pull/539) passes
controlled contention on the first nightly with seeds 0, 1 and 42, and the second
with seed 0. The original fails on both. The same argument issue is tracked in
[parking_lot #542](https://github.com/Amanieu/parking_lot/issues/542); Rust's
[Miri signature change](https://github.com/rust-lang/rust/commit/51dfc2b349ea16ab61e8aec1ad44ed5383d59c21)
also corrected standard-library futex pointer types. These results support the
dependency classification; they do not establish every unpark lifetime or
interleaving. Production dependencies and synchronization remain unchanged.

Strict-provenance contention separately stops at `word_lock.rs:320` with an
**unsupported integer-to-pointer operation**. The default-provenance runs retain
Miri's warning that exposed provenance can hide pointer bugs; no ABI, UB or
aliasing checks were suppressed. Passing the isolated patch does not establish
full thread-safe verification of the unpatched library.

Both ordinary and compiled Froodi controlled reproductions fail with the same
diagnostic on nightly 2026-09-29, seed 0. Their native tests pass. Replay separately
from the focused adapter suite:

```sh
MIRIFLAGS='-Zmiri-symbolic-alignment-check -Zmiri-preemption-rate=1 -Zmiri-seed=0' \
  cargo +nightly-2026-09-29 miri test --locked -p froodi --features compiled \
  --lib compiled::tests::construction::native_concurrent_cached_construction -- --exact
```

Use `compiled::tests::construction::concurrent_cached_construction` for the
compiled variant. Neither a passing focused adapter filter nor reproduction
through ordinary registries proves this shared path sound. The supplied standalone
ABI probe also reproduces the failure on both nightlies; its raw-pointer variant
returns the intended EAGAIN.

Host checks cover `no_std + alloc`, local async and thread-safe parking_lot/Tokio
configurations; they do not certify a bare-metal target. Parsing stays host-side.
The historical native MSRV is not a claim for the opt-in compiled feature, tested
on modern Rust. General concurrent close, cancellation and shutdown questions
remain a separate lifecycle concern.
