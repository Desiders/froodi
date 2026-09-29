# ADR 0004 — Static execution backend

## Status

Accepted

## Context

After rustc links a registry (ADR 0003), every static edge knows its target registration at
compile time. The question is how an edge executes: how much of Froodi's per-edge work
(`TypeId` lookup, map lookup, dynamic dispatch, `Any` downcast, recursive interpretation) can go,
and at what cost in compile time and robustness. `get<T>()` and `get_transient<T>()` must keep
Froodi's semantics (requirements §6).

## Options considered

### Level A — indexed plan

Edges hold registration ids linked by key when the container is built; construction goes through
a table of functions and the value is checked with a downcast. Used for runtime registries.

### Level A+ — table edges with rustc-resolved ids

Static edges call the construction table at the id rustc resolved (a constant). The value is cast
without a check because the slot's type is fixed by that id. Trait bounds name only the target
leaf, so they do not chain through the graph.

### Level B — direct edges

Static edges call the provider's factory directly; rustc can inline the call.

### Level C — typed generated storage

Per-scope typed fields instead of a slot vector.

## Experiments / evidence

`froodi-compile/benches/compare.rs`, criterion, release build, same machine, same graphs for all
engines (`benches/support/generate.py`). Median times:

| Scenario | Froodi | A (indexed) | A+ (table, default) | B (direct) |
|---|---|---|---|---|
| `Container::new`, 100 registrations | 13.5 µs | 19.6 µs | 19.9 µs | 19.4 µs |
| cached `get` | 24.2 ns | 22.1 ns | 22.2 ns | 22.1 ns |
| first `get` of a 100-deep chain | 19.6 µs | 9.9 µs | 7.4 µs | 6.9 µs |
| `enter_build` + resolve a 100-deep request chain | 13.8 µs | 8.7 µs | 6.2 µs | 5.5 µs |
| `get_transient` of a 100-deep transient chain | 6.9 µs | 5.7 µs | 4.4 µs | 2.8 µs |
| first `get` of a 16-wide factory | 528 ns | 460 ns | 501 ns | 447 ns |
| `enter_build` + resolve a captured closure | 164 ns | 119 ns | 119 ns | 126 ns |
| first `get` through a `runtime::<T>()` boundary | — | — | 308 ns | 311 ns |

Compile-time robustness of level B: with a direct call on every edge, rustc must prove each
dependency chain. At about 45 edges (debug and release, in either declaration order) rustc stops
with an internal compiler error while resolving instances; `#![recursion_limit = "512"]` in the
user's crate makes a 100-deep chain compile. A cycle among direct edges is rejected by a trait
overflow (E0275) instead of a dependency path. Level A+ compiles a 100-deep chain and a
500-registration registry under the default limit (`tests/scale.rs`, `tests/registry_scale.rs`),
and reports cycles with paths.

Level C was not built. The slot vector is already indexed by a constant id with an unchecked
cast; typed fields would additionally remove one bounds check and the `Rc<dyn Any>` pointer
width, while making the container type generic (a public typed graph, requirements §9) or erasing
it again at the boundary.

## Decision

- Static edges use level A+ (table edges) by default.
- Level B stays available behind the `direct-edges` feature, for applications whose graphs are
  shallow or that raise `recursion_limit`.
- Runtime registries use level A.
- Level C is not pursued.

## Consequences

### Positive

- Deep, cold and transient resolutions are 1.6–2.6 times faster than current Froodi; cached
  `get` is on par.
- No depth limit and no dependence on `recursion_limit`; cycles are diagnosed with paths.

### Negative

- Container construction costs about 45% more than Froodi's (graph compilation and tables),
  once per container tree.
- Direct calls would be 7–11% faster on cold chains and 35% faster on long transient chains.
- A change to a registry's shape costs seconds of type checking in the application crate: about
  2.4 s for a 100-deep chain and 12 s for 500 registrations, against Froodi's 0.3–0.85 s. Editing a
  provider's body costs the same as in Froodi (`benchmarks.md`, build costs).

### Compatibility impact

None.

## Rejected alternatives

- Level B by default: rustc aborts with an internal error on ordinary graph depths without a
  raised `recursion_limit`, which Froodi avoids by design.
- Level C: see above.

## Follow-up work

- Moving graph compilation out of `Container::new` for registries that are fully static, to
  close the construction gap.
- Profiling where the type checker spends the build time of large registries (`-Zself-profile`)
  and reducing the size of the tree's types.
