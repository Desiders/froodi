# Integration gaps

These existing differences must be addressed when integrating the compile-time
engine into Froodi. The experimental executor remains a backend validation tool.

| Area | Current limitation and integration impact |
|---|---|
| Duplicate registrations | Froodi permits later registrations to replace earlier ones. Static dependencies instead report ambiguous providers; overrides require explicit `RuntimeRegistry::replacing()` fragments. |
| Context-only dependencies | Static parameters need a `context::<T>()` registration so rustc can link them. Public `get` still accepts unregistered context values. |
| Registry-producing functions | Concrete typed trees are generally impractical to name, and opaque return types hide the provider paths. Use a macro or return `RuntimeRegistry`; the latter validates internal edges at container construction. |
| Custom instantiators | Hand-written implementations use parameter-type metadata instead of Froodi's `dependencies()` method. |
| Closed parent with a live child | Froodi preserves parent cache entries copied when the child was created. The compile-time engine resolves wider values through the parent. Closing the parent early needs compatibility work. |
| Async builders | Async resolution and finalization exist, but the builder/context API does not yet match Froodi's full async API. |
| `froodi-auto` | Static descriptors/adapters and their integration remain to be designed. |

## Further work

- Audit soundness of the erased `RegistrationId` execution table: registration
  pointers, safe extension traits, unchecked edge iteration and downcasts, transient storage,
  finalizers, runtime replacements/imports/context, async cancellation, concurrent
  close and container lifetime. Use the [executor contract](architecture.md#safety-contract)
  as the starting point.
- Static cycle checks currently run at code generation for closed compositions of
  at most 1,024 nodes; `cargo check`, opaque compositions and larger graphs retain
  the limitations described in [architecture](architecture.md#bounded-static-cycle-validation).
- Improve ambiguity diagnostics when Rust's diagnostic facilities allow DI-specific
  explanations.

Registration-level overrides and named cross-crate static fragments are possible
future extensions if concrete usage requires them.

## Backend boundary for later integration

Keep the registration frontend, provider index/linker, bounded static validator,
ordered `RegistrationId` topology, and typed instantiator/finalizer adapters.
Froodi's runtime should eventually supply scope ownership, cache/context storage,
construction serialization, close/drop behavior and finalizer scheduling.
No integration is implemented here.

The minimum contract is an immutable plan mapping each ID to its exact provided
Rust type, ordered dependency edges, execution kind and matching adapter. The host
resolves an ID in the owning scope, invokes the adapter with that node's edges,
records the producing ID for finalization, and publishes cached values before
releasing construction serialization. `InjectTransient` bypasses cache/context;
custom resolvers receive a container view and consume no indexed edge. A stable
storage owner must outlive adapters and their borrowing futures.

Current adapters take the experimental concrete `Container`, so connecting them
to Froodi requires a dispatch/container-view adapter; they are not already portable.
Other obstacles include duplicate/override policy, explicit import/context leaves,
per-plan construction locks shared across scopes, separate sync/async dispatch
locks, and import declarations' independent cache setting. Preserve Froodi's
behavior when resolving these differences rather than adopting this executor's
lifecycle as the replacement runtime.

The materialized-edge review checked ordered/repeated requests, resolver skipping,
ID/table alignment, exact `TypeId` replacement/import selection, context insertion,
transient storage and finalizer ownership. Miri found invalid pointer provenance
when a `Box` was moved after executor collection; storage now enters its final
`Rc`/`Arc` allocation before pointers are derived. Raw-edge construction adapters
require `unsafe`; public instantiators return their associated concrete output,
and private linking/collection traits prevent downstream forged plans. This is
not an audit of concurrent close, cancellation or scope lifecycle.

Focused Miri runs passed with nightly 1.101.0 (`c1070d693`, 2026-09-28), both
with `--no-default-features --features async` and with default features plus
`async`. They cover `materialized`, `replacement`, `context_boundary`,
`construction`, and (in local mode) `runtime_registry` and `async_indexed`.
The compile-stage subprocess harness is excluded from Miri and runs under the
normal test suite. For example:

```sh
cargo +nightly miri test -p froodi-compile --no-default-features --features async \
  --test materialized --test construction --test context_boundary \
  --test replacement --test runtime_registry --test async_indexed
```
