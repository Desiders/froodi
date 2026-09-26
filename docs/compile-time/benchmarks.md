# Compile-Time Froodi — Benchmark Plan

## Purpose

The compile-time engine should be selected based on measured trade-offs rather than theoretical elegance.

Benchmarks must answer two questions:

1. Which Froodi runtime costs actually disappear?
2. What compile-time, incremental-build, and binary-size costs replace them?

## 1. Implementations to compare

At minimum compare:

```text
A. Current Froodi runtime implementation

B. Compiled indexed plan
   NodeId/static dependency indices
   runtime factory-value slots

C. Compiled plan + generated typed factory edges

D. Generated typed storage prototype
   only if implemented

E. Mixed static/runtime graph
   once hybrid support exists
```

## 2. Runtime scenarios

### Container/registry construction

Measure startup cost separately from resolution.

### First `get<T>()`

Measure a cold scoped resolution where dependencies are constructed and caches are populated.

### Cached `get<T>()`

Measure repeated access to an already-created scoped dependency.

### `get_transient<T>()`

Measure repeated fresh construction. Transient-heavy graphs may show the strongest benefit from generated direct calls.

### Deep graph

```text
A1 -> A2 -> A3 -> ... -> A100
```

Measures per-edge overhead.

### Wide graph

A root with many independent branches.

### Scope transitions

Measure `enter`, `enter_build`, `with_scope`, `build`, and first access inside a child container.

### Captured closure factory

Ensure runtime factory state does not eliminate static-path gains.

### `instance(value)`

Measure a runtime-provided value participating in an otherwise static graph.

### Mixed static/runtime graph

Measure one explicit runtime dependency embedded in a mostly static graph.

## 3. Runtime cost attribution

Where feasible, separately account for:

```text
TypeId lookup
map lookup
dyn dispatch
Any/downcast
dependency graph traversal
scope traversal
cache lookup
synchronization
factory invocation
```

The purpose is not only to say one version is faster. We need to know why.

## 4. Graph sizes

Suggested generated benchmark graphs:

```text
tiny:    5–10 registrations
medium:  ~50 registrations
large:   ~500 registrations
deep:    chain of 100
wide:    root with many branches
```

Large graphs are especially important for compile-time/code-generation measurements.

## 5. Build-time measurements

Measure:

```text
clean debug build
clean release build
provider body edit
provider signature edit
registry topology edit
scope/config edit
```

A provider-body-only change should ideally not trigger more graph recompilation than necessary.

## 6. Code-size measurements

Record:

```text
generated Rust size
debug binary size
release binary size
number/size of monomorphized resolver paths where practical
```

Compare indexed plans with generated typed calls and typed storage.

## 7. Diagnostic quality

Track whether each architecture produces understandable errors for:

```text
missing binding
cycle
scope violation
ambiguous binding
invalid factory signature
```

An architecture that saves small runtime cost but produces unusable compiler errors may be the wrong choice.

## 8. Decision criteria

The final backend choice should consider:

```text
runtime startup
first resolution
cached resolution
transient resolution
deep-graph overhead
clean compile time
incremental compile time
binary size
generated code size
diagnostics
implementation complexity
Froodi API compatibility
hybrid/runtime flexibility
```

A likely useful architecture may be:

```text
compiled NodeId/static plan
+
generated direct calls for static edges
+
runtime factory-value slots
+
runtime fallback for genuinely dynamic registrations
```

but this remains a hypothesis until benchmarks validate it.
