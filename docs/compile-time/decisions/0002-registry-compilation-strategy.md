# ADR 0002 — Registry compilation strategy

## Status

Accepted

## Context

`registry!` must stay Froodi's composition syntax (requirements §4, §11): unannotated factories
defined elsewhere, closures, captured closures, `instance(value)`, `config`, `finalizer`,
`extend`, and a path to `async_registry!`. The macro sees `provide(make_service)` as tokens and
never sees the provided or dependency types. The question is how the static graph is obtained
and checked.

## Options considered

### A — typed macro expansion, linked by rustc (hybrid)

A proc macro turns the clauses into a balanced tree of typed leaves (ADR 0001). rustc links
every dependency by type (`Has<T, I>`, ADR 0003); the graph compiler checks what rustc cannot
(cycles, scopes, unused duplicates) when the container is built.

### B — procedural macro with runtime linking (indexed)

The same macro and tree, but no rustc linking: dependencies carry `TypeId` keys and are linked
by the graph compiler when the container is built. Implemented as `RuntimeRegistry`
(`into_runtime()`).

### C — proc macro + `build.rs`

A `build.rs` parses the sources, extracts registrations and generates graph code.

### D — Fundle-like typed carrier

Fundle declares components as fields of a struct and encodes dependencies as trait bounds on a
type-state builder (see `fundle-analysis.md`).

## Experiments / evidence

| Criterion | A: typed + rustc | B: indexed | C: `build.rs` | D: Fundle-like |
|---|---|---|---|---|
| Named factories unannotated | yes | yes | only when the function is in a scanned file with a written signature | no: types are written in the component struct |
| Factory defined apart from registration | yes | yes | partly | no |
| Inline closures | yes | yes | only with written parameter and return types | as setters |
| Captured closures keep runtime state | yes, stored inline | yes, stored inline | no: generated code cannot capture | yes |
| `instance(value)` runtime | yes | yes | type not written | yes |
| `extend(...)` | typed nesting | by key | runtime expression, not analysable | nesting + forwarding lists |
| rustc validates type relationships | yes | at startup (`TypeId`) | no: string spellings | yes |
| What `build.rs` does | nothing | nothing | parse, resolve, generate | nothing |
| Missing binding | compile error | startup diagnostic | build-time, for the subset it can see | compile error |
| Cycles | graph compiler at startup | graph compiler at startup | build-time | unwritable |
| Diagnostics | rustc error naming the parameter; graph paths with origins | graph paths with origins | its own | rustc trait errors |
| Cross-crate composition | macros; erased fragments by key (ADR 0005) | yes | per-crate scanning only | nested structs |

Evidence in the code:

- A and B run the same scenarios (`compat.rs`, `runtime_registry.rs`, `scale.rs`); a 100-deep
  chain resolves in both under the default `recursion_limit`.
- A: 500 registrations in one `registry!` build and resolve under the default
  `recursion_limit` (`registry_scale.rs`, after Froodi's test of the same name).
- C: see the `froodi-compile-build` prototype and its tests; its results are summarised below.
- D: `fundle-analysis.md`, from Fundle's source and a real expansion.

### `build.rs` staging

A `build.rs` runs before its crate is compiled. It can read source text but not the types rustc
infers, and it runs before the same package's proc macros expand, so it cannot consume their
output. The prototype in `froodi-compile-build` measures what it can still recover.

`froodi_compile_build::analyze` parses `registry!` / `async_registry!` invocations with `syn`
and recovers a registration's provided type and dependencies only from what is written: a
function in the same file with a `Result`-typed signature, a closure with annotated parameters and
a written return type, `instance::<T>(..)`. Everything else gets a stated reason.

Measured on this workspace (`froodi-compile-build/tests/sample.rs`):

| Source | Registrations | Resolved | Main reasons |
|---|---|---|---|
| `examples/` | 26 | 0 | instance type not written 10, closure parameter not annotated 8, closure return type not written 8 |
| `froodi/tests/` | 42 | 4 | closure return type not written 37 |
| `froodi/benches/` | 148 | 0 | closure return type not written 148 |

It also cannot see registrations produced by other macros: in `examples/sync_auto_provide` the
container gets three registrations from `#[injectable]`, and the source text shows one
(`froodi-compile-build/tests/staging.rs`). Type identity from text is spellings only: `Config`,
`crate::Config`, `settings::Config` and a `type` alias are reported as unresolvable rather than
guessed.

## Decision

Option A, with option B as its runtime mode. `registry!` and `async_registry!` are proc macros
that build a typed tree; rustc links static edges; the graph compiler compiles the whole graph
once when a container is built; runtime fragments reuse the same tree, IR and compiler with
key linking. No `build.rs` step.

## Consequences

### Positive

- Froodi's syntax and factory model are kept; no second declaration language.
- Missing and ambiguous providers of static dependencies are compile errors.
- One IR and one compiler serve static and runtime registrations.

### Negative

- Ambiguity errors are rustc's E0283 text.
- A change to a registry's shape costs seconds of type checking in the application crate: about
  2.4 s for a 100-deep chain and 12 s for 500 registrations, against Froodi's 0.3–0.85 s. Editing a
  provider's body costs the same as in Froodi (`benchmarks.md`, build costs).
- Cycle and scope checks run at container construction, not at compile time; a const-evaluated
  check would be reported by `cargo build` but not `cargo check`, because it runs after
  monomorphization.

### Compatibility impact

None to the syntax. Behaviour changes are recorded in ADR 0003 (duplicates) and ADR 0006
(context-only types in static factories, explicit overrides).

## Rejected alternatives

- C: it sees spellings, not types, cannot keep captured runtime state, and adds a build step
  that must be kept in sync with the macro.
- D as the public model: it would replace `registry!` with component structs.

## Follow-up work

- Rendering rustc's ambiguity error in DI terms, if `#[diagnostic]` attributes gain that power.
