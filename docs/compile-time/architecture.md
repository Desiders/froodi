# Compile-Time Froodi — Working Architecture

## Status

This is a **working architecture**, not a final ADR.

Several implementation mechanisms still require experiments, especially around `registry!`, type identity, build staging, and cross-crate composition.

## 1. Architectural objective

Preserve the Froodi programming model while compiling structurally stable graph work ahead of runtime.

```text
Froodi-style declarations
        ↓
registry!/async_registry! frontend
        ↓
registration IR
        ↓
graph compiler
        ↓
CompiledGraph / StaticPlan / generated Rust
        ↓
Froodi-like runtime Container
        ↓
get<T>() / get_transient<T>()
```

Runtime state remains for:

```text
factory values
captured closures
instance(value)
scope instances
cache contents
Context
finalizer state
genuinely dynamic registrations
```

## 2. Experimental crate boundary

The new engine should initially be isolated in new crates.

Working split:

```text
froodi-compile
    public experimental facade

froodi-compile-core
    registration IR
    graph IR
    graph validation
    diagnostics
    compilation transforms

froodi-compile-runtime
    Container
    scope/cache state
    factory-value slots
    Context
    finalizer state
    compiled-plan execution
    future runtime fallback

froodi-compile-macros
    registry!/async_registry! frontend experiments
    generated glue

froodi-compile-build
    optional build-time compiler experiments
```

Only the separation between current Froodi and the experimental implementation is intentional. The internal crate split may change.

## 3. Registration is the core graph unit

The graph should be built around registrations rather than only Rust types.

Conceptually:

```text
Registration
    id
    provided binding
    factory slot
    dependency requests
    scope
    config
    finalizer
    execution kind
    source origin
```

A dependency request preserves its mode:

```text
DependencyRequest
    target
    mode = shared/scoped | transient | other supported resolver mode
```

This is necessary to preserve `Inject<T>` vs `InjectTransient<T>`.

## 4. Separate static structure from runtime values

A compiled registration may conceptually look like:

```text
registration 12
    provides Repository
    scope = Request
    dependencies = [Database via Inject]
    factory slot = 12
```

The graph information may be static.

The actual value stored in `factory slot 12` may be a function item, closure, captured closure, or `instance(value)` wrapper.

This is the key mechanism that may let Froodi retain current factory ergonomics while still compiling topology.

## 5. `Instantiator<Deps>` as a possible bridge

The existing `Instantiator<Deps>` abstraction should be reused or adapted if experiments show that it can expose enough type information for dependencies, provided type, and error type while retaining the concrete factory value.

This is preferred over inventing a new mandatory provider declaration language.

## 6. Registry frontend remains undecided

Candidate approaches:

### Procedural `registry!`

Pros:
- sees full registration syntax;
- can generate source metadata/glue;
- good diagnostics for syntax-level concerns.

Unknowns:
- semantic knowledge about arbitrary instantiator expressions;
- cross-crate composition;
- interaction with runtime-captured closure values.

### Typed macro expansion

Pros:
- rustc sees concrete factory types;
- may preserve `Instantiator<Deps>` information naturally;
- no separate source-language parser.

Risks:
- large nested types;
- trait solver cost;
- difficult diagnostics;
- monomorphization/build-time growth.

### `build.rs` hybrid

Pros:
- explicit global graph compilation/code generation;
- natural place for graph algorithms.

Risks:
- Cargo staging;
- metadata transport;
- potential duplicate parsing;
- current-crate semantic information is unavailable before crate compilation.

### Fundle-like compiler/type-system encoding

Potentially valuable if it lets rustc validate actual Rust types and generate static calls without a complex separate semantic frontend.

### Hybrid

A likely direction may combine macro-generated typed descriptors with graph compilation/code generation, but no approach should be selected without prototypes.

## 7. Graph compiler responsibilities

For static registrations, the compiler should aim to precompute:

```text
registration indexing
dependency topology
missing binding validation
duplicate/ambiguous registration validation
cycle detection
scope accessibility validation
reachability
dependency ordering
structural finalization relationships
```

The graph compiler should produce deterministic output.

## 8. Runtime execution levels

### Level A — compiled indexed plan

```text
public get<T>()
    ↓
registration lookup
    ↓
NodeId
    ↓
dependency NodeIds
    ↓
runtime factory slot
```

Some erasure may remain.

### Level B — generated typed factory edges

```text
public get<T>()
    ↓
generated/static dispatch
    ↓
generated direct resolver/factory calls
```

Static dependency edges can potentially avoid `TypeId`, map lookup, dyn dispatch, and `Any/downcast`.

### Level C — generated typed storage

Scope/cache state may be represented as generated typed fields.

This should only be adopted if benchmarks justify the extra compiler/code-size complexity.

## 9. Public dispatch vs internal static edges

One promising design is:

```text
container.get::<T>()
    ↓
one type-based/public dispatch
    ↓
compiled node/generated resolver
    ↓
internal dependency edges use NodeId/direct calls
```

This preserves ergonomic public lookup without paying the same dynamic lookup cost on every dependency edge.

Whether `TypeId` remains at this outer boundary is an implementation detail to benchmark.

## 10. Scope/runtime model

Even with a compiled graph, runtime must maintain:

```text
current scope/container
parent/child relationship
cache contents
Context
which registrations were instantiated
finalizer state
factory runtime values
```

The compiler should precompute scope ownership/accessibility where possible.

Caching remains controlled independently through Froodi's configuration semantics.

## 11. Registry fragments

The preferred composition model is not one monolithic global component.

Investigate fragments that carry:

```text
static registration metadata
+
runtime factory values
```

and can be combined through Froodi-like `extend(...)`.

Composition should trigger graph validation across fragments when the result remains statically analyzable.

## 12. Static/runtime coexistence

The future architecture should allow:

```text
static registrations
+
runtime registrations
```

behind one Froodi-like `Container`.

Static guarantees must remain explicit.

A missing static dependency must not silently fall through to an arbitrary runtime registry unless the composition explicitly models that boundary.

Do not introduce a new `Dynamic<T>` public wrapper until experiments demonstrate that registration-level metadata is insufficient.

## 13. Async/finalizers

The same graph representation should eventually describe sync factory, async factory, sync finalizer, and async finalizer.

Do not create unrelated sync and async graph systems.

## 14. Open architectural decisions

The following require ADRs after experiments:

1. `registry!` compilation mechanism.
2. How `Instantiator<Deps>` is reused/adapted.
3. Type identity and rustc validation strategy.
4. Whether `build.rs` is required.
5. Static plan vs generated typed edges.
6. Whether typed generated storage is worthwhile.
7. Registry-fragment representation.
8. Explicit static/runtime boundary semantics.
9. Cross-crate metadata/composition.
10. How `froodi-auto` feeds the static engine.
