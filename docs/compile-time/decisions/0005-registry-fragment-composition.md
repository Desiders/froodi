# ADR 0005 — Registry fragment composition

## Status

Accepted

## Context

Froodi composes registries with `extend(...)`, and applications build fragments in functions or
other crates (requirements §11). In the compile-time engine a registry is a typed tree
(ADR 0001) linked by rustc (ADR 0003). The fragment's type carries the static information, so
how a fragment crosses a function or crate boundary decides whether its registrations stay
statically checked.

## Options considered

### Option A — fragments as typed values

`let infrastructure = registry! { ... };` then `registry! { ..., extend(infrastructure) }`.
The fragment's tree nests into the outer tree; `Has` searches the whole tree, so every
dependency across fragments is linked by rustc.

### Option B — functions returning `Registry<impl Trait>`

`fn infrastructure() -> Registry<impl RegistryTree>`. The return type is opaque: outside the
function rustc cannot see the tree's structure, so `Has` cannot search it.

### Option C — erased fragments (`RuntimeRegistry`)

`registry! { ... }.into_runtime()` erases the tree behind a trait object. The fragment has a
nameable type and crosses any boundary. Its registrations are linked by key when the container
is built.

### Option D — macros as fragment producers

`macro_rules! infrastructure { () => { registry! { ... } } }`, exported with `#[macro_export]`.
The fragment stays a typed value at the use site.

## Experiments / evidence

- `froodi-compile/tests/compat.rs`: `extend` of typed fragments, of several fragments and of
  `registry!()` runs on both engines.
- Writing `fn registry() -> Registry<impl ...>` for `tests/resolver.rs` failed: a container built
  from the opaque tree cannot link a static dependency. The test uses a local macro instead
  (option D).
- `froodi-compile/tests/runtime_registry.rs`: a fragment returned from a function as
  `RuntimeRegistry` resolves, depends on static registrations, and reports a missing binding or
  a cycle with its path when the container is built.
- `froodi-compile/tests/scale.rs`: a 100-registration chain resolves as a typed fragment and as a
  runtime fragment.

## Decision

- Fragments built and composed in one expression or function stay typed (option A) and are
  fully checked by rustc.
- A fragment that must be named — returned from a function, exported from a crate, chosen at
  runtime — is a `RuntimeRegistry` (option C). `extend(...)` accepts both kinds.
- Reusable statically checked fragments are `macro_rules!` producers (option D). The tests use
  one within a crate; across crates the same macro is exported with `#[macro_export]` and must
  name `registry!` by its full path.
- Option B is not supported: it looks static but loses static linking.

## Consequences

### Positive

- `extend(...)` keeps Froodi's syntax for every kind of fragment.
- Where a fragment is erased, its checks move to container construction with the same
  diagnostics; nothing becomes a silent runtime lookup (ADR 0006).

### Negative

- A registry-producing function loses compile-time checks of its internal edges.
- Erased fragments execute through the indexed backend.

### Compatibility impact

Froodi code that returns `Registry` from functions ports by returning `RuntimeRegistry` and
adding `.into_runtime()`, or by turning the function into a macro.

## Rejected alternatives

- Option B: rustc hides the structure static linking needs.
- A type-name registry built by a macro or `build.rs`: it would decide type identity from
  spellings (ADR 0003).

## Follow-up work

- Cross-crate static fragments beyond macros, if generic const expressions or similar features
  make tree types nameable.
