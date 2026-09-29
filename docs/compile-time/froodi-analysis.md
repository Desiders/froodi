# Froodi Analysis for the Compile-Time Engine

## Purpose

This document captures the parts of the current Froodi design that materially constrain or enable a compile-time/static backend.

It is intentionally focused on the programming model and runtime responsibilities relevant to the new engine.

It should be expanded with exact source references as implementation work proceeds.

## 1. Core user model

The important high-level Froodi flow is:

```text
factory definitions
    ↓
registry! / async_registry!
    ↓
Registry
    ↓
Container
    ↓
get<T>() / get_transient<T>()
```

The compile-time effort should change the middle of this pipeline without casually changing the top or bottom.

Target transformation:

```text
CURRENT

Froodi API
    ↓
registry! creates runtime graph metadata
    ↓
runtime validation / graph interpretation
    ↓
Container resolution
    ↓
factory execution

TARGET

same or nearly same Froodi API
    ↓
registry!/compiler frontend
    ↓
compile-time graph analysis
    ↓
compiled/generated execution representation
    ↓
Froodi-like Container
    ↓
factory execution
```

## 2. `Instantiator<Deps>` is a major architectural asset

Confirmed from source (`froodi/src/instantiator.rs`):

- `Instantiator<Deps>: Clone + 'static` has `type Provides`, `type Error: Into<InstantiateErrorKind>`,
  `fn instantiate(&mut self, Deps)` and `fn dependencies() -> BTreeSet<Dependency>`.
- A blanket impl covers `F: FnMut(T1..Tn) -> Result<R, E> + Clone + 'static` for up to sixteen
  parameters, each `Ti: DependencyResolver`. So `Deps` is always the parameter tuple, and named
  functions, closures and captured closures all implement it without annotations.
- `instance(value)` is `move || Ok(value.clone())`: a captured closure with no dependencies.
- At registration Froodi erases the factory into a boxed service
  (`boxed_instantiator`): `Box<dyn Service<Container, Box<dyn Any>>>`. Each call clones the
  factory, resolves `Deps` from the container, calls it and boxes the result as `Box<dyn Any>`.
- `dependencies()` reports each parameter's `DependencyResolver::type_info()`; for `Inject<T>`
  and `InjectTransient<T>` that is `T`, for any other resolver it is the resolver type itself.

What generic code bounded by `F: Instantiator<Deps>` knows statically, for every factory kind:
the parameter tuple, the provided type and the error type. What stays runtime: the factory value,
including a captured environment. The experimental engine keeps the trait and stores the factory
value in a typed registration instead of a boxed service (ADR 0001).

## 3. Factory declaration is separate from registration

A normal target pattern is:

```rust
fn make_repository(
    Inject(database): Inject<Database>,
) -> InstantiatorResult<Repository> {
    ...
}
```

with registration elsewhere:

```rust
registry! {
    scope(App) [
        provide(make_repository),
    ],
}
```

Registration adds concerns such as scope, `Config`, finalizer, and composition.

Those concerns should not be pushed into the factory definition merely to simplify compile-time processing.

The compile-time engine should also preserve, where technically possible:

```text
named functions
inline closures
captured closures
instance(value)
```

## 4. Dependency requests carry execution semantics

Froodi dependency edges are not just type references.

```text
Inject<T>            DependencyResolver::resolve -> container.get::<T>()
InjectTransient<T>   DependencyResolver::resolve -> container.get_transient::<T>()
custom resolver      arbitrary code over &Container
```

Custom resolvers exist in Froodi itself: `MapInject<T>` in the dptree integration reads `T` from
a `DependencyMap` the container provides. Their real dependencies are invisible from the type.
Froodi's graph validation cannot see them either: the key it records is the resolver type, which
no registration provides, so cycle and scope checks skip the edge.

The experimental IR therefore has three request modes: `Shared`, `Transient`, and `Resolver`
for custom resolvers, which is recorded but never resolved or reported missing.

## 5. `get<T>()` and `get_transient<T>()` are separate operations

The new backend should preserve both.

### `get<T>()`

Conceptually:

```text
locate registration
locate owning scope
consult cache according to Config
construct if necessary
record finalizer/runtime state
return Froodi shared-pointer form
```

### `get_transient<T>()`

Conceptually:

```text
locate registration
construct fresh value
do not use the normal provided-value cache
return T
```

Generated static edges must retain this difference.

## 6. Scope and caching are distinct

Froodi has registration configuration including cache behavior.

Therefore:

```text
scope ownership
```

and:

```text
cache provided result?
```

must remain separate fields in the compiled representation.

## 7. Registry composition matters

Froodi's registry model is compositional.

Patterns conceptually include:

```rust
registry! {
    scope(App) [
        provide(database),
    ],

    extend(other_registry),
}
```

The compile-time engine should therefore investigate **compiled registry fragments** instead of assuming one global monolithic component declaration.

Important questions include:

- can fragments carry static metadata and runtime factory values together?
- can two fragments be validated after composition?
- when does arbitrary runtime control flow make a fragment dynamic?
- how could this eventually compose across crates?

## 8. Runtime values do not imply a runtime graph

`instance(value)` is an important example.

The value may only exist at runtime:

```rust
provide(App, instance(config))
```

but graph structure may still know:

```text
provides Config
dependencies = none
scope = App
```

Captured closures have the same architectural lesson.

The static compiler should not assume that only compile-time constants are eligible for a static graph.

## 9. Container lifecycle and scopes remain runtime state

Even with a precompiled graph, runtime still needs to represent:

- current container/scope instance;
- parent/child relationships;
- cache contents;
- actual instantiated values;
- `Context`;
- finalizer state;
- runtime factory values.

The compiler can precompute structure. It cannot eliminate the runtime lifecycle.

## 10. Candidate runtime work to move earlier

Likely compile-time/precompiled candidates for static registrations:

```text
dependency topology
cycle detection
missing binding checks
duplicate/ambiguous registration checks
scope accessibility validation
registration indexing
reachability
dependency ordering
static finalization relationships
```

Other work may be reducible rather than removable:

```text
TypeId lookup
map lookup
dynamic factory dispatch
Any/downcast
scope lookup
```

The most important performance opportunity is likely reducing repeated work on internal dependency edges, not merely moving one-time validation out of startup.

## 11. Work that inherently remains runtime

Even a highly static backend still needs runtime handling for:

```text
captured factory values
instance(value)
actual cache contents
fallible construction results
Context state
current scope/container
which providers were actually instantiated
finalizer execution
genuinely dynamic registrations
plugins / runtime modules
```

## 12. Static execution levels worth prototyping

### Level A — compiled indexed graph

Use compact IDs:

```text
NodeId
ScopeId
dependency NodeIds
factory slots
```

Some erasure may remain.

### Level B — generated typed factory edges

Potential benefits:

```text
less TypeId lookup
less map lookup
less dyn dispatch
less Any/downcast
better inlining
better dead-code elimination
```

### Level C — generated typed storage

This may remove more erasure, but can increase generated code, compile time, monomorphization, binary size, and architecture complexity.

It should be benchmarked rather than assumed.

## 13. Important open questions

1. Can `Instantiator<Deps>` remain substantially unchanged?
2. Can ordinary functions remain completely unannotated?
3. Can captured closures remain fully supported?
4. Can `registry!` expose enough static information without a second declaration language?
5. Can `extend(...)` remain composable and statically analyzable?
6. How should actual Rust type identity be validated?
7. Is `build.rs` needed at all?
8. How much runtime overhead disappears with `NodeId` alone?
9. Are generated typed factory calls worth their compile-time/code-size cost?
10. Can static and dynamic registrations coexist behind one `Container` without weakening static guarantees?

## 14. Registry validation and duplicates in current Froodi

From `froodi/src/registry.rs` and its tests:

- `registry!` calls `Registry::validate()` and unwraps it, so an invalid registry panics where it
  is built. `validate` checks cycles over every dependency and that no registration depends on a
  narrower scope (`detect_unreachable_scopes`).
- A missing dependency is not validated. It surfaces as `ResolveErrorKind::NoInstantiator` when
  resolved. One reason: a child container's `Context` can supply values no registration provides.
- Entries live in a `BTreeMap<TypeInfo, InstantiatorData>`. A later registration of the same type
  replaces the earlier one, and `extend` (always the last clause) overrides local entries. Tests
  pin this: `test_registry_extend_later_registry_overrides_duplicate_entry`,
  `test_registry_extend_overrides_duplicate_entry_from_previous_macro_invocation`.
- Every registry carries an entry for `Container` itself, in the root scope, with
  `cache_provides = false`, because caching the container in its own cache would keep it alive.

## 15. Runtime-work accounting

Classes, as in issue #58:

```text
A  fully compile-time            rustc or the macro does it; nothing remains at runtime
B  structure precomputed          done once when the container is built; runtime keeps state
C  inherently runtime             depends on values or on what was resolved
D  depends on registration mode   differs between static and runtime registrations
```

| Operation | Current Froodi | Compile-time engine | Class |
|---|---|---|---|
| Registry construction | `BTreeMap` of boxed services, built when `registry!` runs | typed tree value, no allocation per registration (factories stored inline) | A |
| Dependency metadata | `BTreeSet<Dependency>` per registration | parameter types; IR built once at container construction | A (static) / B |
| Registry merging (`extend`) | map union, later entries win | tree nesting at compile time; runtime fragments appended by id | A / D |
| Missing binding | found on resolution (`NoInstantiator`) | compile error for static edges; startup diagnostic for runtime registries | A / D |
| Duplicate binding | silently replaced | compile error when depended on; startup diagnostic otherwise | A / B |
| Cycle validation | DFS over the map when `registry!` runs | graph compiler, once per container tree | B |
| Scope validation | when `registry!` runs | graph compiler, once per container tree | B |
| `TypeInfo`/`TypeId` lookup | per `get` and per dependency edge | once per public `get`; none on static edges | B |
| Dependency resolver traversal | `Deps::resolve(&container)` → `container.get::<T>()` per edge | table call by constant id (static), id by key at startup (runtime) | A / D |
| Factory dispatch | `Box<dyn Service>` call | construction table call; direct call with `direct-edges` | B |
| `Any`/downcast | per resolution | none on static edges; one check on runtime edges and at the `get` boundary | A / D |
| Scope traversal | parent walk to the owning priority | parent walk to the owning level (precomputed per node) | B / C |
| Cache lookup | `BTreeMap<TypeInfo, Rc<dyn Any>>` per container | `Vec` slot by id | C (state) |
| Per-type construction lock | `TypeId`-keyed lock map, created lazily | one lock per registration, allocated with the plan | B |
| Finalizer bookkeeping | resolved list per container, finalizer looked up by type | resolved list per container, finalizer by id | C |
| Context propagation | context map merged into each child's cache | visible context per container; registered types written into slots | C |

What moves to compile time is the graph's structure. What stays runtime is state: which values
exist, in which container, with which finalizers pending, and the context each container sees.

## 16. Scopes, cache and context (confirmed from `froodi/src/container.rs`)

- `Container::new` starts at the widest scope and descends through the scopes skipped by
  default, keeping each as the parent of the next (`build_root`). `new_with_start_scope` stops at
  the requested priority.
- A child built by `enter()` / `with_scope` / `with_context` descends from the next scope; the
  first new level keeps its parent open (`close_parent = false`), intermediate levels close their
  parent.
- `get` checks the container's cache, then walks to the registration's owning scope, resolves
  there, and caches the result in the requesting container too when `cache_provides` is set.
- A registration of a narrower scope fails with `NoAccessible`.
- A child's cache starts as a copy of the parent's cache map plus the new context; context values
  therefore answer `get` for any type, registered or not, and take precedence over factories.
- `get_transient` walks to the owning scope and builds there; it ignores cache and context.
- `close` runs the finalizers of values constructed in that container, newest first, then resets
  the cache to the container's context, and closes the parent when `close_parent` is set.
  `Drop` calls `close`.
- The container itself is registered in the root scope with `cache_provides = false`, so
  `get::<Container>()` returns the root container.

Differences in the compile-time engine: a child does not copy the parent's cache at creation, it
resolves wider values through the parent when first asked. The observable result differs only
after a parent is closed while a child is still in use: Froodi's child keeps the parent's values
it had copied, the new engine's child resolves them again.

## 17. Finalizers and async (confirmed from `froodi/src/async_impl`)

- `async_registry!` builds an async registry; sync registries join it through `extend`, and the
  async container embeds a sync container that it falls back to for types the async registry
  lacks. An async factory can depend on sync registrations; a sync factory cannot reach async
  ones.
- Async factories and finalizers are functions returning futures. Froodi boxes every async
  factory and finalizer call.
- `close().await` awaits async finalizers newest first, then closes the embedded sync container.
  `Drop` of an async container does not run async finalizers; Froodi logs a warning with the
  number of pending finalizers.
- A value whose construction failed is not in the resolved list, but the dependencies it built
  are, so they are finalized on close.

The compile-time engine keeps one graph for both: async leaves sit in the same tree and IR, and
the async container shares all state with the sync one.
