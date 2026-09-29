# Compile-Time Froodi — Compatibility Matrix

## Purpose

This document tracks whether the experimental engine preserves the current Froodi programming model.

A feature should not be considered complete merely because an equivalent capability exists under a different API.

## Status legend

```text
PRESERVE
    target is the same API and semantics

ADAPT INTERNALLY
    public behavior should remain, implementation changes substantially

EXPERIMENT
    compatibility mechanism is not yet proven

RUNTIME ONLY
    expected to remain a runtime capability

OPEN
    requires research before committing to a design

CHANGED
    public behavior differs from Froodi; the row names the ADR that explains why
```

## Matrix

| Froodi capability | Target | Compile-time/static treatment | Runtime work remaining | Status |
|---|---|---|---|---|
| `Container` | Preserve concept/API | Plan shared per container tree: compiled graph, linked tree, construction tables | Scope/cache/context/finalizer state | PRESERVE |
| `Container::new` | Preserve | Same scope descent as Froodi; the graph is compiled here, `new` panics on diagnostics like `registry!` | Runtime values and caches | PRESERVE (`try_new` added) |
| `Container::new_with_start_scope` | Preserve | Same | Actual scope instance | PRESERVE |
| `Container::get<T>()` | Preserve exact terminology | One `TypeId` lookup at the boundary; static edges need none | Cache, construction, context | PRESERVE (compat.rs) |
| `Container::get_transient<T>()` | Preserve exact terminology | Built in the owning scope, cache and context ignored | Factory invocation | PRESERVE (compat.rs) |
| `enter` | Preserve | Scope levels precomputed | Child container state | PRESERVE |
| `enter_build` / `with_scope` / `build` | Preserve where practical | Same descent and errors as Froodi | Builder/runtime scope selection | PRESERVE |
| `close` | Preserve | Finalizers by id, newest first; intermediate parents closed | Which values were instantiated; actual finalization | PRESERVE (compat.rs); a child resolves parent values again after the parent is closed, where Froodi keeps its copy |
| `registry!` | Preserve syntax aggressively | Proc macro builds a typed registration tree; rustc links it (ADR 0001, 0003) | Factory values | PRESERVE: same clauses, same tests pass on both engines (`compat.rs`) |
| `async_registry!` | Preserve | Same tree and IR, `Async` execution kind | Futures/runtime values | PRESERVE (async_compat.rs, vertical slice) |
| `provide(scope, factory)` | Preserve | Scope stored in the typed leaf; described in the IR | Concrete factory value | PRESERVE |
| `scope(scope) [ provide(...) ]` | Preserve | Same | Runtime scope state | PRESERVE |
| `config = ...` | Preserve | `Config` stored per registration; `cache_provides` in the IR, independent of scope | Runtime expression evaluated once, at registration | PRESERVE |
| `finalizer = ...` | Preserve | Finalizer stored in the leaf's type (`WithFinalizer`); presence in the IR; `NoFinalizer` is zero-sized | Actual finalizer execution | PRESERVE (execution: #61) |
| `extend(...)` | Preserve | Typed fragments nest into one tree; `RuntimeRegistry` fragments join by key (ADR 0005) | Factory values of every fragment | CHANGED (ADR 0003): a type provided twice and depended on is a compile error instead of "last one wins"; replacement becomes explicit (#62) |
| `instance(value)` | Preserve | Leaf with no dependencies; IR source `Instance` | Actual `value` | PRESERVE |
| Named function factory | Preserve unannotated use | `Instantiator<Deps>` exposes deps, provided type, error | Zero-sized function item | PRESERVE |
| Inline closure | Preserve | Same as named function; parameter types annotated as in Froodi | Closure value | PRESERVE |
| Captured closure | Preserve | Dependency shape from its type | Captured environment stored inline, not boxed | PRESERVE |
| `Instantiator<Deps>` | Prefer preserving core abstraction | Kept: `Provides`, `Error`, `instantiate`; `dependencies()` replaced by parameter metadata | Factory invocation/state | CHANGED (ADR 0001): hand-written impls drop `dependencies()` |
| `DependencyResolver` | Preserve extension semantics where feasible | Custom resolvers are `Resolver` requests: recorded, never linked | Resolver code runs against the container | PRESERVE: same trait shape; `Inject`/`InjectTransient` are static edges, not resolvers |
| `Inject<T>` | Preserve | Static edge by registration id (table call; direct call with `direct-edges`) | Cache/value state | PRESERVE |
| `InjectTransient<T>` | Preserve | Static transient edge, built in the owning scope | Fresh factory invocation | PRESERVE |
| `Config::cache_provides` | Preserve | Stored per registration, independent of scope | Actual cache contents | PRESERVE (compat.rs) |
| `Scope` | Preserve | Same trait | Scope instances | PRESERVE |
| `Scopes<N>` | Preserve | Hierarchy fixed per registry; scope rules checked by the graph compiler | Runtime selected scope instance | PRESERVE |
| `DefaultScope` | Preserve | Same enum | Runtime lifecycle | PRESERVE |
| Custom scopes | Preserve | Same as `DefaultScope`: priority, name, skipped by default | Runtime lifecycle | PRESERVE (compat.rs custom scope scenario) |
| `Context` | Preserve behavior | Registered types written into cache slots; `context::<T>()` declares a context-only type for static factories | Actual context state | PRESERVE for `get`; static factories need `context::<T>()` (ADR 0006) |
| Sync factory | Preserve | Static graph + construction table | Factory state/result | PRESERVE |
| Fallible factory | Preserve | Same error nesting as Froodi | Actual error/result | PRESERVE |
| Async factory | Preserve | Same graph, direct awaits on static edges | Future/result | PRESERVE (vertical slice) |
| Sync finalizer | Preserve | Finalizer stored in the leaf | Runtime execution | PRESERVE |
| Async finalizer | Preserve | Same; runs on `close().await`, not on drop, like Froodi | Async execution | PRESERVE (vertical slice) |
| thread-safe mode | Preserve | `Arc`, selected lock backend, one construction lock per registration | Runtime synchronization | PRESERVE |
| non-thread-safe mode | Preserve | `Rc`/`RefCell` | Runtime Rc-style state | PRESERVE (single_threaded.rs) |
| `no_std + alloc` direction | Preserve where current Froodi supports it | Core is `no_std`; runtime builds `no_std` with `lock-spin` | Platform runtime | PRESERVE |
| `froodi-auto` | Preserve as optional mechanism | Generate static descriptors/adapters in future | Dynamic/linkme path may remain for runtime mode | OPEN |
| Runtime plugins/modules | Preserve future capability | `RuntimeRegistry`, linked by key at startup | Dynamic registry | RUNTIME ONLY (ADR 0005, 0006) |
| Test overrides | Preserve future capability | `RuntimeRegistry::replacing()` | Override selection | CHANGED (ADR 0006): explicit, where Froodi let a later registration win |
| Dynamic registration | Preserve future capability | Runtime registries and `runtime::<T>()` boundaries | Runtime registry | RUNTIME ONLY (ADR 0006) |

## Compatibility policy

Any row moving away from `PRESERVE` must explain:

```text
what current Froodi behavior exists
why exact preservation is difficult/impossible
what alternatives were tested
what API change is proposed
how future integration into Froodi is affected
```

A new backend being easier to implement is not enough reason to change the public model.

## Remaining incompatibilities

Each follows the compatibility policy above.

### Duplicate registrations and overrides

- Froodi: a later registration of a type replaces the earlier one, including through `extend`.
- Why: rustc decides which registration a static dependency targets; two providers of one
  depended-on type are ambiguous to it (ADR 0003).
- Tested: unmarked duplicates (compile error or startup diagnostic), explicit replacement.
- API: `RuntimeRegistry::replacing()` marks overrides.
- Integration: Froodi code that relies on "last one wins" must mark its overrides.

### Context-only types in static factories

- Froodi: a factory may depend on any type a child container's `Context` supplies, registered
  or not.
- Why: rustc must find a provider for every static parameter.
- Tested: `context::<T>()` declaration; `get` of unregistered context values keeps working.
- API: `provide(scope, context::<T>())` per such type.
- Integration: an integration that injects request data through `Context` needs one
  `context::<T>()` per injected type in the registry it builds.

### Registry-producing functions

- Froodi: `fn infrastructure() -> Registry`.
- Why: the typed tree is unnameable; an `impl Trait` return type hides it from rustc (ADR 0005).
- Tested: runtime registries returned from functions; `macro_rules!` producers.
- API: return `RuntimeRegistry` (`.into_runtime()`), or turn the function into a macro.
- Integration: a runtime registry loses compile-time checks of its internal edges; they run when
  the container is built.

### Hand-written `Instantiator` impls

- Froodi: the trait has `dependencies()`.
- Why: dependency metadata comes from parameter types (ADR 0001).
- Tested: every factory kind through the blanket impl.
- API: hand-written impls drop the method.
- Integration: only code implementing `Instantiator` by hand.

### A closed parent under a live child

- Froodi: a child keeps the parent's cached values it copied at creation, even after the parent
  is closed.
- Why: a child does not copy the parent's cache; it resolves wider values through the parent.
- Tested: every scope scenario in `compat.rs` that does not close a parent before its child.
- API: none.
- Integration: only code that closes a parent while a child is still in use.

### Async

- Froodi: a full async container with its own builders.
- Why: the prototype implements a vertical slice (factories, finalizers, `get`, `get_transient`,
  `close`, `enter_build`, `enter_build_with_scope`) on the shared graph model.
- Tested: `async_compat.rs`, `end_to_end.rs`.
- API: async children are built with `enter_build` and `enter_build_with_scope(scope)`; they
  take a scope, and the context of their parent.
- Integration: completing the async builders on the same shared state.
