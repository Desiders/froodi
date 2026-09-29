# Compile-Time Froodi — Working Architecture

## Status

This document describes the experimental engine as implemented. Decisions and their evidence
live in the ADRs under `decisions/`.

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

## 6. Registry frontend

`registry!` and `async_registry!` are proc macros in `froodi-compile-macros` (ADR 0002). They
parse Froodi's clauses and emit a balanced tree of typed leaves:

```text
registry! { scope(App) [ provide(a), provide(b) ], provide(Request, c), extend(r) }
    ->
Registry::from_tree(Node(Node(reg(App, a, ..), reg(App, b, ..)), Node(reg(Request, c, ..), r_tree)), [App, Request])
```

The macro never needs a type: the provided type and the parameters come from the factory's
`Instantiator<Deps>` impl. It records each registration's origin (the expression, file, line,
column) for diagnostics and recognises `instance(...)` to mark the value source. A balanced tree
keeps trait resolution depth at `log2(n)`: 500 registrations need no `recursion_limit`.

`extend(...)` accepts a typed registry, whose tree joins the outer tree, or a
`RuntimeRegistry`, which joins as an opaque node (ADR 0005).

## 7. Graph compiler responsibilities

Checks are split between rustc and the graph compiler (ADR 0003).

rustc, while linking the registry tree in `Container::new`:

```text
which registration each Inject<T> / InjectTransient<T> targets
missing provider of a static dependency        compile error
ambiguous provider of a static dependency      compile error (E0283)
sync factory depending on an async one         compile error
```

With the `direct-edges` feature a cycle among static edges is also a compile error: proving that
the factories can call each other never terminates, and rustc reports a trait overflow instead
of a dependency path. With the default table edges the cycle reaches the graph compiler.

`froodi-compile-core::compile`, once, when `Container::try_new` / `Container::new` builds a
container:

```text
duplicate bindings no factory depends on
dependency cycles, whatever the request mode          (Froodi rule)
dependency on a narrower scope                       (Froodi rule)
scopes outside the hierarchy
requests rustc did not resolve (Target::Key)
registration indexing, construction order, reachability
```

Diagnostics name the registrations involved with their source locations, and render missing
bindings and cycles as dependency trees. `Container::new` panics with them, as Froodi's
`registry!` panics on a failed validation; `Container::try_new` returns them.

## 8. Execution

`Container::new` links the tree (rustc proves every static edge), describes the linked tree in
the IR with the ids rustc resolved, compiles the graph, and builds one construction table per
registration in declaration order:

```text
Entry
    item       pointer to the leaf inside the boxed tree
    construct  fn(root, item, &Container, id) -> Rc<dyn Any>     `get` path
    transient  fn(root, item, &Container, id, out)                `get_transient` path
    finalize   fn(item, Rc<dyn Any>)
```

Three edge kinds exist (ADR 0004):

```text
table edge (default)      static Inject<T>: container.shared(ID) with ID a constant from rustc;
                          the value is cast without a check, the slot's type is fixed
direct edge (direct-edges) static Inject<T>: the provider's factory is called directly
indexed edge              runtime registries: ids linked by key at startup, value downcast checked
```

Table edges keep trait bounds shallow, so graph depth is unlimited and cycles reach the graph
compiler. Direct edges let rustc inline the call but make it prove every dependency chain; at
about forty edges this exceeds the default recursion limit and rustc stops with an internal
compiler error.

## 9. Public dispatch and internal edges

```text
container.get::<T>()
    one TypeId -> id lookup in the compiled graph (binary search)
    cache slot of id in this container, or construction
internal edges
    no TypeId, no map, no downcast check (static) / one downcast check (runtime registries)
```

## 10. Scope and runtime model

A container has a scope level, a parent and per-registration cache slots:

```text
Container -> Inner
    plan       shared by every container of one tree: compiled graph, linked tree, tables,
               one construction lock per registration (thread_safe)
    level      position of the scope in the sorted hierarchy
    slots      Vec<Option<Rc<dyn Any>>> by registration id
    context    Context values visible here: the parent's, overridden by its own
    resolved   values constructed here that have a finalizer, in order
    parent, close_parent
```

`get` on a registration owned by a wider scope resolves in that ancestor and keeps a copy in the
current cache, as Froodi does; a narrower scope is `NoAccessible`. `cache_provides` decides only
whether a slot is written. Context values of registered types are written into the slots when a
container is created; unregistered ones answer `get` from the context map. `get_transient` builds
in the owning container and ignores the cache and the context.

Thread safety follows Froodi's features: `Rc`/`RefCell` without `thread_safe`, `Arc` and the
selected lock backend with it; `no_std + alloc` builds with `lock-spin`.

## 11. Registry fragments

Typed fragments nest into one tree and are linked by rustc across fragments. A fragment that
must be named is erased into a `RuntimeRegistry` and linked by key when the container is built
(ADR 0005).

## 12. Static/runtime coexistence

The boundary is declared in the composition (ADR 0006): `runtime::<T>()` and `context::<T>()`
registrations, runtime registries, and `replacing()` for overrides. A static dependency on a
type only a runtime registry provides does not compile without a declaration.

## 13. Async and finalizers

`async_registry!` builds the same tree with async leaves; the IR marks them `Async`. The async
container wraps the sync one and adds an async construction table. Async factories await static
dependencies directly; futures are boxed at the `get` boundary and when resolving in an
ancestor. Finalizers run newest first on `close`; async ones only on `close().await`. A sync
factory depending on an async registration is a compile error.

## 14. Decisions

```text
ADR 0001  factory model: Instantiator<Deps>, factory values stored in typed leaves
ADR 0002  registry frontend: proc macro building a typed tree, linked by rustc
ADR 0003  type identity: rustc; the graph compiler compares keys only where rustc cannot link
ADR 0004  static execution backend
ADR 0005  registry fragment composition
ADR 0006  static/runtime boundary
```

Open: how `froodi-auto` feeds the static engine, and integration into Froodi itself.
