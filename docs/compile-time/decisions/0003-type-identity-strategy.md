# ADR 0003 — Type identity strategy

## Status

Accepted

## Context

The graph compiler must know which registration a dependency request targets. In Rust the same
type can be written as `Foo`, `crate::Foo`, `module::Foo` or through an alias. Deciding identity
from spellings would mean reimplementing name resolution (requirements §15).

`registry!` sees only `provide(make_service)`. The provided type and the parameter types of a
factory are never tokens the macro can read.

## Options considered

### Option A — rustc decides identity through trait resolution

`registry!` builds a typed tree of registrations. The trait `Has<T, I>` finds the leaf providing
`T`, with the path `I` inferred by rustc. Linking runs `Has` for every `Inject<T>` and
`InjectTransient<T>` parameter.

### Option B — `TypeId` keys compared at runtime

The IR carries `TypeId` binding keys; the graph compiler matches them when the container is
built.

### Option C — type-name strings

A macro or a `build.rs` compares written type names.

## Experiments / evidence

- `froodi-compile/tests/type_identity.rs`: a provider written through an alias satisfies a
  dependency written as a full path, and `get::<settings::Config>()` finds it.
- `froodi-compile/tests/ui/duplicate_through_alias.stderr`: two providers of one type, one
  written through an alias, are rejected as an ambiguity naming `Has<settings::Config, _>`.
- `froodi-compile/tests/ui/missing_binding.stderr`: a missing provider fails with
  `no registration provides the factory parameter Inject<Config>`.
- The graph compiler (`froodi-compile-core`) is generic over its key and only compares keys; it
  also accepts edges rustc already resolved (`Target::Id`).

## Decision

The division of responsibility is:

```text
rustc
    type identity, aliases, trait bounds
    which registration a static Inject<T> / InjectTransient<T> targets
    missing providers and ambiguous providers among the ones a factory depends on

graph compiler
    topology over registration ids
    scope rules, cycles, reachability, construction order
    duplicates no factory depends on, and requests rustc did not resolve
```

Where the registry is fully visible to rustc, edges enter the compiler already resolved
(`Target::Id`). Where it is not (runtime registrations, opaque fragments), requests carry
`TypeId` keys (`Target::Key`), which the compiler only compares. Type-name strings are used only
in diagnostics.

## Consequences

### Positive

- No reimplementation of Rust name resolution; aliases and paths behave as rustc says.
- Missing providers of static dependencies are compile errors.

### Negative

- An ambiguous provider is reported by rustc as `type annotations needed` (E0283). The message
  names the type and every registration's location, but not in DI terms.
- A duplicate that no factory depends on is not seen by rustc; the graph compiler reports it
  when the container is built.

### Compatibility impact

Froodi lets a later registration of a type replace an earlier one (for example through
`extend`). With rustc deciding identity, two providers of a type that is depended on cannot
coexist in a static registry. Replacing a registration becomes an explicit operation (issue #62).

## Rejected alternatives

- Type-name strings: `Foo` and `crate::Foo` would be different bindings and an alias would be a
  third one.

## Follow-up work

- Explicit replacement of a registration, for test overrides (issue #62).
