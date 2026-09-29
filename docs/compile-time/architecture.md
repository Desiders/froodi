# Compile-time engine architecture

```text
registry! / async_registry!
    ↓
balanced typed registration tree
    ↓
rustc links Inject<T> / InjectTransient<T> to registrations
    ↓
paths become RegistrationId graph edges
    ↓
indexed registration executors
```

## Crates

| Crate | Responsibility |
|---|---|
| `froodi-compile` | Public facade, compatibility tests and benchmarks |
| `froodi-compile-macros` | Registry syntax to balanced typed trees |
| `froodi-compile-runtime` | Linking, construction, containers and lifecycle |
| `froodi-compile-core` | Registration IR, graph compiler and diagnostics; `no_std + alloc` |
| `froodi-compile-build` | Source analysis for investigating build-script staging |

The runtime depends on the core; the facade exposes the runtime and macros.
The existing `froodi` implementation remains separate and supplies the comparison
baseline in compatibility tests and benchmarks.

## Typed linking

`Reg<Out, Inst, Deps, Fin>` stores an instantiator, finalizer and metadata inline.
`Instantiator<Deps>` exposes the provided type and dependency tuple without invoking
the instantiator. Captured closures and `instance(value)` remain runtime values.
The macro builds a balanced tree to keep registry traversal bounds shallow.

| Abstraction | Role |
|---|---|
| `ProviderPath<T, Path>` | Proves an unlinked provider of `T`; rustc infers `Path` |
| `RegistrationPath<Path>` | Exposes the linked registration type and constant index |
| `LinkDependency` / `LinkDependencies` | Infers provider paths for instantiator parameters |
| `LinkedInject<Path>` / `LinkedInjectTransient<Path>` | Carries a path and its dependency mode |
| `Linked<Out, Inst, Deps, Fin, Links>` | Stores a registration with linked parameters |
| `LinkedDependencyMetadata` / `LinkedDependenciesMetadata` | Converts paths to IR requests containing IDs |
| `ConstructRegistration` | Invokes the instantiator with resolved parameters |
| `ResolveLinkedDependency` / `ResolveLinkedDependencies` | Resolves parameters through indexed executors |
| `CollectExecutors` | Collects executors in registration order |

Provider lookup and linked index lookup remain separate: merging them introduced
redundant trait errors for missing registrations. Neither performs runtime tree
traversal. `SyncProvider` checks that a target supports synchronous construction
without recursively requiring construction of its dependencies. Async counterparts
use the same shallow bounds, so dependency depth does not determine trait-resolution
depth.

Custom `DependencyResolver` parameters use `ByResolver` and `RequestMode::Resolver`.
They run user code against the container and have no statically validated target.
`RequestMode::Inject` and `RequestMode::InjectTransient` describe the two built-in
request modes. Caching is a separate registration setting.

## Graph compilation

The IR is registration-based: declaration order determines `RegistrationId`, and
requests target either an ID already linked by rustc or a type key to resolve at
container construction. Metadata includes scope, caching, execution kind, value
source, finalizer presence and diagnostic origin.

Rustc detects missing and ambiguous static providers and rejects synchronous
instantiators that depend on async registrations. Container construction checks
cycles, scope accessibility, invalid scope hierarchies and remaining duplicate or
missing runtime bindings. `try_new` returns diagnostics; `new` panics on them.
Graph compilation preserves registration IDs.

Typed `extend(...)` fragments join the tree and link across fragment boundaries.
`RuntimeRegistry` fragments join as opaque nodes and link by `TypeId` during graph
compilation. Static dependencies crossing that boundary declare `runtime::<T>()`;
context-only dependencies declare `context::<T>()`. Explicit `replacing()` fragments
redirect matching registrations while preserving the provided Rust type.

## Indexed execution

`Plan::build_with` boxes the linked tree before collecting executor pointers.
Static registrations precede runtime fragments in matching graph/table order.
Each `RegistrationId` selects a `RegistrationExecutor` containing:

- an erased pointer to the concrete registration;
- specialized construction and transient-construction functions;
- a matching finalizer function.

This is the boundary between heterogeneous Rust registration types and the
homogeneous runtime table. Both public requests and dependency edges use it.
Public `get<T>()` first maps `TypeId` to an ID. Static edges already carry constant
IDs, allowing unchecked cached-value casts and typed transient output storage.
Runtime edges use IDs linked at startup and checked casts. Async construction uses
an indexed async table; its erased transient output is checked when extracted.

### Safety contract

The source contract lives beside `RegistrationExecutor` in
[`graph.rs`](../../froodi-compile-runtime/src/graph.rs) and also applies to async execution:

- The boxed tree and owned runtime fragments remain live at stable addresses until
  all executors and borrowing futures are gone. `Plan` drops tables before storage.
- Each registration pointer stays paired with functions specialized for its exact
  registration type and, where used, the same concrete `Root` type.
- IDs consistently index graph nodes, executors, type metadata and cache slots.
  Redirects select a complete target executor; graph changes must preserve this pairing.
- A slot for a registration providing `T` contains exactly `T`, permitting unchecked
  casts. Runtime, replacement, import, context and ancestor-cache paths preserve that type.
- Sync transient output points to aligned, writable, uninitialized storage for the
  exact provided type and is read only after success. Async output uses an owned box.
- Finalizers receive values produced by their matching registration, recorded under
  the actual producing ID after redirects. Close/drop retain the plan during finalization.
- Async construction cannot outlive borrowed storage; finalizer futures own their
  cloned finalizer and value. Thread-safe plans require `Send + Sync` trees and
  synchronized access.

These obligations still require a dedicated soundness review; documenting them
is not a completed audit.

## Runtime semantics

Containers share a plan and own scope state, context, indexed cache slots and
finalizer records. Wider-scope registrations resolve in the owning ancestor;
narrower-scope requests fail with `NoAccessible`.

`Inject<T>` follows `get<T>()` semantics. `cache_provides: false` causes fresh
construction independently of dependency mode. `InjectTransient<T>` follows
`get_transient<T>()`: construct in the owning scope, bypassing cache and context.
Context values for registered types populate slots; public `get` can also read
unregistered context values.

Finalizers run newest first for values constructed through `get`, including when
caching is disabled. Async finalizers require explicit `close().await`.
Thread safety follows Froodi's feature selection (`Arc` and locks, or
`Rc`/`RefCell`); `lock-spin` supports `no_std + alloc` builds.
