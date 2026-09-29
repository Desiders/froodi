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
