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

Froodi's factory model already separates factory behavior from registration.

A factory can be a normal function or closure whose type participates in an `Instantiator<Deps>` implementation.

The relevant information is conceptually:

```text
Deps
Provides
Error
```

This is useful because a compile-time engine may be able to know the dependency shape from types while retaining the concrete factory value at runtime.

That matters for captured closures.

Conceptual split:

```text
factory type / dependency structure
    compile-time useful

factory value / captured environment
    runtime state
```

This should be investigated before inventing a new provider API.

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

Two especially important forms are:

```rust
Inject<T>
InjectTransient<T>
```

Conceptually:

```text
Inject<T>
    -> get<T>()-like semantics

InjectTransient<T>
    -> get_transient<T>()-like semantics
```

Therefore the compile-time graph should preserve dependency request mode rather than flattening every edge to a plain type relation.

Custom `DependencyResolver` support should also be inspected before freezing the IR.

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
