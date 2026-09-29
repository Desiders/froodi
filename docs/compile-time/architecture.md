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

The new engine lives in its own crates, next to the current ones in the same workspace:

```text
froodi-compile            public facade: re-exports the runtime and `registry!`;
                          examples and compatibility tests import this crate
froodi-compile-core       registration IR, graph compiler, diagnostics;
                          `no_std + alloc`, no `Any`, no containers
froodi-compile-runtime    Container, typed registration tree, linking and execution,
                          scopes, Context, errors; follows Froodi's `std`/`thread_safe` features
froodi-compile-macros     `registry!` proc macro: Froodi syntax in, typed tree out
froodi-compile-build      home of `build.rs` staging experiments
```

Dependencies point one way: `froodi-compile` depends on the runtime and the macros, the runtime
depends on the core. None of them depends on `froodi`; only the compatibility test of
`froodi-compile` uses `froodi`, to run each scenario against both engines
(`froodi-compile/tests/compat.rs`).

The boundary between current Froodi and the experiment is intentional. The split inside the
experiment may change.

## 3. Registration is the core graph unit

The graph is built around registrations, not types. The IR lives in `froodi-compile-core`
(`ir.rs`):

```text
Graph<K>
    registrations: Vec<Registration<K>>     declaration order = RegistrationId
    scopes: Vec<ScopeKey>                    the whole hierarchy (Scopes::all())

Registration<K>
    key: K                   binding key of the provided type (TypeId in the runtime)
    type_name                diagnostics only
    requests: Vec<DependencyRequest<K>>
    scope: ScopeKey          priority, name, skipped_by_default
    cache_provides: bool     Config, independent of the scope
    finalizer: Option<ExecutionKind>
    execution: ExecutionKind Sync | Async
    source: ValueSource      Factory | Instance | Context | Runtime | Container
    origin: Option<Origin>   the instantiator expression, file, line, column

DependencyRequest<K>
    target: Target<K>        Id(RegistrationId) resolved by rustc | Key(K) resolved by the compiler
    mode: RequestMode        Shared (Inject) | Transient (InjectTransient) | Resolver (custom DependencyResolver)
    type_name                diagnostics only
```

`Inject<T>` and `InjectTransient<T>` stay distinct edges. A custom `DependencyResolver` is
recorded as a `Resolver` request: it is user code reading the container at runtime, so it has no
static target, creates no edge and is never reported as missing. Froodi's own graph treats it the
same way: its key never matches a registration.

`Registry::graph()` produces this IR from any registry, in declaration order, with extended
fragments numbered after local registrations. The output is deterministic
(`froodi-compile/tests/ir.rs`).

## 4. Static structure and runtime values

A registration is a typed leaf of the registry tree:

```text
Reg<Provides, Factory, Deps, Finalizer>
    factory: Factory         the function item, closure or instance(value) wrapper, stored inline
    finalizer: Finalizer     NoFinalizer (zero-sized) | WithFinalizer<F>
    meta: scope, Config, value source, origin
```

The leaf's position in the tree is its factory slot. Its type carries the whole static structure
(`Provides`, `Deps`); its value carries the runtime state (the factory, a captured environment,
an instance). Nothing is erased when a registration is created (ADR 0001).

## 5. `Instantiator<Deps>` is the bridge

`Instantiator<Deps>` keeps Froodi's shape: `Provides`, `Error`, `instantiate(&mut self, Deps)`.
Its blanket impl covers functions and closures of up to sixteen parameters. Generic code bounded
by `F: Instantiator<Deps>` knows the dependency tuple, the provided type and the error type
without calling the factory (`froodi-compile/tests/factory_model.rs`). The runtime-set
`dependencies()` method of Froodi's trait is gone: parameter types describe themselves
(`DepMeta`), and linking resolves them.

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

Checks are split between rustc and the graph compiler (ADR 0003).

rustc, while linking the registry tree in `Container::new`:

```text
which registration each Inject<T> / InjectTransient<T> targets
missing provider of a static dependency        compile error
ambiguous provider of a static dependency      compile error (E0283)
cycle among static edges                       compile error (E0275, trait overflow)
```

The cycle error is a side effect of direct calls between factories: proving that the factories
can call each other never terminates. It rejects the registry, but its message is rustc's
overflow report, not a dependency path (`froodi-compile/tests/ui/cycle.stderr`).

`froodi-compile-core::compile`, once, when `Container::try_new` / `Container::new` builds a
container:

```text
duplicate bindings no factory depends on
dependency cycles that reach it, whatever the mode   (Froodi rule)
dependency on a narrower scope                       (Froodi rule)
scopes outside the hierarchy
requests rustc did not resolve (Target::Key)
registration indexing, construction order, reachability
```

Diagnostics name the registrations involved with their source locations, and render missing
bindings and cycles as dependency trees. `Container::new` panics with them, as Froodi's
`registry!` panics on a failed validation; `Container::try_new` returns them.

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
