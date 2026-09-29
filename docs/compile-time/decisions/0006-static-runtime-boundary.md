# ADR 0006 — Static/runtime boundary

## Status

Accepted

## Context

Froodi must keep runtime capabilities: plugins, runtime-selected registries, test overrides,
conditional composition, application-provided values (requirements §10). A runtime fallback
must not weaken static guarantees: a missing dependency in a graph intended to be static stays
an error unless the composition declares a runtime boundary. Factory parameters should stay
`Inject<T>`; a `Dynamic<T>` marker should be avoided unless registration-level metadata is
insufficient.

## Options considered

### Option A — factory-level markers (`Dynamic<T>`)

The dependent factory declares that a dependency is runtime-provided.

### Option B — registration- and composition-level declarations

The composition says where runtime values enter; factories keep `Inject<T>`.

### Option C — automatic fallback

A static dependency nobody provides statically is looked up at runtime.

## Experiments / evidence

`froodi-compile/tests/runtime_registry.rs`, `context_boundary.rs`, `replacement.rs` and
`tests/ui/static_needs_runtime_boundary.rs`:

| Direction | Mechanism | When a provider is missing |
|---|---|---|
| static → static | rustc links `Inject<T>` to the provider | compile error |
| static → runtime | `provide(scope, runtime::<T>())` declares the boundary; the graph compiler links it to the runtime provider | compile error without the declaration; startup diagnostic with it |
| static → context | `provide(scope, context::<T>())` declares a value that arrives through `Context` | `ResolveErrorKind::NoContextValue` when resolved without it |
| runtime → static | the graph compiler links by key | startup diagnostic |
| runtime → runtime | the graph compiler links by key | startup diagnostic |
| replacement | `RuntimeRegistry::replacing()` marks registrations that take over their type, including the edges rustc linked to the replaced one | an unmarked second registration is a duplicate |

## Decision

Option B. The boundary is declared where the graph is composed:

- `runtime::<T>()` and `context::<T>()` are registrations. They give a static graph a
  provider of `T` whose value arrives at runtime, so factories keep `Inject<T>`.
- `RuntimeRegistry` is the unit of runtime composition (ADR 0005), and `replacing()` is the only
  way to override a registration.
- There is one container API: `get::<T>()` and `get_transient::<T>()` serve static and runtime
  registrations alike.

## Consequences

### Positive

- A static graph never falls through to a runtime lookup by accident.
- Test overrides are explicit and visible in the composition.
- Every runtime link is still checked before the first resolution, when the container is built.

### Negative

- Porting Froodi code that relies on context values without registering them requires a
  `context::<T>()` declaration per type.
- Overriding a registration needs `replacing()`, where Froodi let a later registration win.

### Compatibility impact

`get` still returns an unregistered context value, as in Froodi. Only static factories need the
declaration, because rustc must find a provider for their parameters.

## Rejected alternatives

- Option A: it moves a composition concern into factory signatures, which Froodi keeps separate.
- Option C: it silently weakens static guarantees.

## Follow-up work

- A registration-level override for single entries, if `replacing()` on whole fragments proves
  too coarse in practice.
