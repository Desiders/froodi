# Fundle Analysis for Froodi

## Purpose

Fundle is a compile-time dependency injection crate. It is a secondary implementation reference
for the compile-time Froodi engine.

The question this document answers:

> Which Fundle implementation mechanisms help compile Froodi's existing programming model?

It does not ask how Froodi should adopt Fundle's API.

## Source

- Repository: `microsoft/oxidizer`, crates `fundle`, `fundle_macros`, `fundle_macros_impl`.
- Commit analysed: `ca3a3720afafb51d79b405aa08b3220f4a9c80c0` (2026-09-25), published as `fundle 0.4.0`.
- Size: about 1,100 lines of macro implementation (`bundle.rs` 818, `deps.rs` 125, `newtype.rs` 103)
  and a runtime crate of four marker structs and three traits (`fundle/src/lib.rs`, `exports.rs`).

## 1. Programming model

A Fundle application declares one struct whose fields are its components:

```rust
#[fundle::bundle]
pub struct AppState {
    logger: Logger,
    database: Database,
}

let app = AppState::builder()
    .logger(|_| Logger::new())
    .database(|x| Database::new(x))
    .build();
```

A component declares what it needs through a `#[fundle::deps]` struct and accepts
`impl Into<ThatDeps>`:

```rust
#[fundle::deps]
struct DatabaseDeps {
    logger: Logger,
}

impl Database {
    fn new(deps: impl Into<DatabaseDeps>) -> Self { ... }
}
```

Every component is built eagerly, exactly once, in the order of the setter calls. The result is a
plain struct. There are no scopes, no caches, no lazy resolution and no container at runtime.

## 2. Mechanisms, from the source

### 2.1 Macro architecture

`#[bundle]` is a single attribute proc macro (`fundle_macros_impl/src/bundle.rs`, `bundle()`).
It reads the struct's named fields and emits, into a private module `_AppState`:

- `AppStateBuilder<RW, LOGGER, DATABASE>`: one `Option<T>` per field and one type parameter per
  field, each `fundle::Set` or `fundle::NotSet` (`generate_builder_struct`);
- one setter per field, callable only while that field is `NotSet`; it returns the builder with
  that field `Set` (`generate_setter_impls`). Each setter comes in four variants: plain, `_try`,
  `_async`, `_try_async`;
- `AsRef<FieldType>` on the builder, implemented only when that field is `Set`
  (`generate_as_ref_impls`);
- `build()`, implemented only when every field is `Set` (`generate_build_impl`);
- `Export<N>` impls and a `select` `macro_rules!` for fields whose type occurs more than once.

`#[deps]` (`deps.rs`) emits `From<T> for DatabaseDeps where T: AsRef<Logger> + ...`; every field is
filled with `AsRef::as_ref(&value).to_owned()`.

### 2.2 Representative expansion

Expanded with `rustc -Zunpretty=expanded` from the example above (abridged):

```rust
pub struct AppStateBuilder<RW, LOGGER, DATABASE> {
    logger: Option<Logger>,
    database: Option<Database>,
    _phantom: PhantomData<(RW, LOGGER, DATABASE)>,
}

impl<DATABASE> AppStateBuilder<Write, NotSet, DATABASE> {
    pub fn logger(self, f: impl Fn(&<Self as Writer>::Reader) -> Logger)
        -> AppStateBuilder<Write, Set, DATABASE> { ... }
}

impl<RW, DATABASE> AsRef<Logger> for AppStateBuilder<RW, Set, DATABASE> {
    fn as_ref(&self) -> &Logger { self.logger.as_ref().unwrap() }
}

impl AppStateBuilder<Write, Set, Set> {
    pub fn build(self) -> AppState { ... }
}

impl<T> From<T> for DatabaseDeps where T: AsRef<Logger> {
    fn from(value: T) -> Self { Self { logger: value.as_ref().to_owned() } }
}
```

### 2.3 Provider and dependency representation

- A provider is the closure passed to a setter. It can capture anything; captured state is used
  once, at construction.
- A dependency is a trait bound: `AsRef<Logger>` on the builder in its current type state.
- The graph exists only as that type state. There is no graph data structure anywhere, at compile
  time or at runtime.

### 2.4 Type identity

The provided type of every component is written in the struct, so the macro sees it as tokens.
Fundle decides which types are unique by comparing token strings (`quote!(#field_type).to_string()`
in `bundle()`), and generates `AsRef` only for unique ones.

That comparison is not type identity. `Logger` and `crate::Logger` are different strings. Fundle
still stays sound because it only uses the string to *decide which impls to emit*: two fields that
are the same type under different spellings produce two `AsRef` impls for one type, and rustc
rejects the conflict (E0119). rustc remains the authority on identity.

### 2.5 Missing binding, ordering and cycles

Asking for a component before it is set fails trait resolution:

```text
error[E0277]: the trait bound `&AppStateBuilder<Read, NotSet, NotSet>: Into<DatabaseDeps>` is not satisfied
help: the trait `AsRef<Logger>` is not implemented for `AppStateBuilder<Read, NotSet, NotSet>`
```

Cycles cannot be expressed: a setter only sees fields set before it, so declaration order is a
topological order the user writes by hand. Fundle never detects a cycle; it makes one unwritable.

### 2.6 Duplicates

Two fields of the same type are allowed. `AsRef` is not generated for that type, so a dependency
on it fails like a missing binding. The user disambiguates with the generated
`AppState!(select(x) => Logger(logger_1))` macro.

### 2.7 Generated code and runtime

A setter call is a closure call and a struct move. `AsRef` is a field access plus `unwrap` on an
`Option` that the type state has already proven `Some`. Everything monomorphizes; there is no
`TypeId`, no map, no `dyn`, no `Any`. The price is that every component is constructed, always, at
build time.

### 2.8 Scopes, fallibility, async

- Scopes: none. The bundle is one lifetime.
- Fallible providers: `_try` setters return `Result<Builder, E>`.
- Async providers: `_async` setters take an `AsyncFn`; the builder is threaded through `.await`.
  No boxing is needed because construction is a linear sequence of awaits.

### 2.9 Cross-module and cross-crate composition

A bundle is an ordinary struct, so it crosses crates as a type. Composition is nesting: a field
may itself be a bundle, and `#[forward(A, B)]` re-exports its inner `AsRef<A>`, `AsRef<B>` on the
outer builder (`generate_forwarded_as_ref_impls`). Forwarded types must be listed by hand because
of rustc issue #51445, noted in `fundle/src/exports.rs`.

### 2.10 Diagnostics and build cost

- Errors are rustc trait errors that name `AsRef<T>` and the builder's type state. They are
  precise about *which* type is missing, but they talk about builder generics, not about
  components.
- Generated code grows with fields × fields: each setter and each `AsRef` impl repeats every
  type parameter. For `n` fields the macro emits `O(n)` impls of `O(n)` size.
- Trait resolution is shallow: every bound is a direct impl match on the builder.

## 3. Mapping to Froodi

| Fundle technique | What it solves | Equivalent Froodi problem | Works with Froodi's model? | Recommendation |
|---|---|---|---|---|
| Dependencies as trait bounds on a typed carrier | rustc proves every dependency exists | Missing-binding detection without reimplementing type resolution | Yes, if the carrier is the registry tree built by `registry!` instead of a user-declared struct | **Borrow.** The prototype uses `Has<T, I>` over the registration tree. |
| Provided type written in the declaration | macro sees types as tokens | `registry!` sees only `provide(make_service)`, never the type | No: Froodi factories are declared elsewhere and unannotated | **Do not copy.** Froodi takes the provided type from `Instantiator::Provides` instead. |
| Token-string uniqueness, rustc as final judge | decides which impls to emit | Duplicate detection | Partly: Froodi has no tokens for types; rustc's ambiguity error gives the same guarantee | **Borrow the principle** (strings never decide identity). |
| Type-state builder, declaration order = topological order | makes cycles unwritable | Cycle detection | No: Froodi resolves lazily in any declaration order | **Do not copy.** Cycles are found by the graph compiler. |
| Eager construction of every component | removes runtime lookup entirely | `get`/`get_transient`, scopes, caching | No: Froodi constructs lazily per scope and supports transient values | **Do not copy.** |
| Monomorphized direct construction | zero dispatch cost | Static dependency edges without `TypeId`/map/`dyn`/`Any` | Yes, inside one registry: an edge is a path known to rustc | **Borrow.** Level B backend calls the dependency's factory directly. |
| Nested bundle + `#[forward]` | composing across modules and crates | `extend(...)` of registry fragments | Partly: a fragment is a typed value, so it composes by value, not by forwarding lists | **Adapt.** `extend` nests fragment trees; no forwarding list is needed because lookup searches the whole tree. |
| `AsyncFn` setters, no boxing | async construction | `async_registry!` | Partly: Froodi's async resolution is lazy and recursive | **Adapt** only the observation that a linear chain of awaits needs no boxing. |

For each Froodi constraint:

```text
Instantiator<Deps>?          yes: the dependency tuple is already a type; bounds attach to it
registry!?                   yes: registry! builds the typed carrier Fundle's struct plays
separate factory definition? yes: nothing needs to be annotated
captured closures?           yes: the closure value is stored in the carrier, as in Fundle
instance(value)?             yes: a closure returning a clone
get/get_transient?           yes, with a runtime container on top; Fundle has none
custom scopes?               not from Fundle; scopes remain Froodi's runtime model
```

## 4. Special focus

### Rust type checking

Fundle confirms that rustc can own type identity while the DI layer owns topology. Froodi goes
further than Fundle because Froodi's macro never sees types at all. The prototype in
`froodi-compile-runtime/src/graph.rs` shows it is still enough: `registry!` builds a balanced tree
of registrations, and `Has<T, I>` finds the provider of `T` by type with the path `I` inferred.

- A missing provider fails with `no registration provides `T``
  (`#[diagnostic::on_unimplemented]` on `Has`).
- Two providers of one type fail as an ambiguity (E0283).
- 500 registrations resolve under the default `recursion_limit`, because the tree depth is
  `log2(n)`.

### Generated direct calls

Fundle's zero-cost path relies on eager construction. Froodi keeps lazy scoped resolution, so an
edge still needs a cache check. What carries over is the edge itself: after linking, an
`Inject<T>` parameter resolves through a path rustc already chose, so the call to the provider's
factory is a monomorphized direct call.

One limit appears that Fundle does not have. When every edge is a direct call, proving that the
whole tree can execute makes rustc walk dependency chains. If providers are declared after their
dependents, a chain of about one hundred edges exceeds the default `recursion_limit`. Declared in
the other order, a chain of 500 compiles. Fundle avoids this because its bounds never chain. An
indexed backend, where edges go through a table of construction functions, has no such chain; the
choice between the two belongs to the static execution backend decision (#59).

### Build pipeline

Fundle needs no `build.rs`: everything happens in one proc macro plus trait resolution. Froodi's
prototype follows the same pattern. The typed tree plus rustc covers missing and duplicate
bindings; the graph compiler in `froodi-compile-core` covers cycles and scope rules. A `build.rs`
step would not see the provided types of unannotated factories, because those types exist only
after rustc type-checks the crate.

### Cross-crate composition

Fundle crosses crates by nesting a public struct and listing forwarded types by hand. A Froodi
registry fragment is a value whose type is the registration tree. Nesting one tree inside another
keeps every registration visible to `Has`, which searches the whole tree, so composition needs no
forwarding list. How fragments are named across function and crate boundaries belongs to the
registry composition work (#62).

## 5. Conclusion

Fundle shows that Rust's trait system can validate a dependency graph with no external tooling
and with rustc as the only authority on type identity. Froodi borrows that principle and the
direct-call execution of static edges.

Fundle's public model does not fit Froodi: the component struct, eager construction, type-state
ordering and the absence of scopes, caching and transient values. Froodi keeps `registry!`,
unannotated factories and `get`/`get_transient`, and uses its own registration tree as the typed
carrier instead of a user-declared struct.
