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
| `Container` | Preserve concept/API | Use compiled graph/static execution metadata | Scope/cache/context/factory state | PRESERVE |
| `Container::new` | Preserve | Construct runtime state from compiled/registered plan | Runtime values and caches | PRESERVE |
| `Container::new_with_start_scope` | Preserve | Scope ownership can be precomputed | Actual scope instance | PRESERVE |
| `Container::get<T>()` | Preserve exact terminology | Static dispatch/index/direct resolver where possible | Cache/value construction/runtime fallback | PRESERVE |
| `Container::get_transient<T>()` | Preserve exact terminology | Generated transient construction path | Factory invocation | PRESERVE |
| `enter` | Preserve | Static scope relationships can be precomputed | Child container state | PRESERVE |
| `enter_build` / `with_scope` / `build` | Preserve where practical | Scope validation may move earlier | Builder/runtime scope selection | PRESERVE |
| `close` | Preserve | Structural finalizer relationships may be compiled | Which values were instantiated; actual finalization | PRESERVE |
| `registry!` | Preserve syntax aggressively | Proc macro builds a typed registration tree; rustc links it (ADR 0001, 0003) | Factory values | PRESERVE: same clauses, same tests pass on both engines (`compat.rs`) |
| `async_registry!` | Preserve | Same graph model with async execution metadata | Futures/runtime values | OPEN |
| `provide(scope, factory)` | Preserve | Scope stored in the typed leaf; described in the IR | Concrete factory value | PRESERVE |
| `scope(scope) [ provide(...) ]` | Preserve | Same | Runtime scope state | PRESERVE |
| `config = ...` | Preserve | `Config` stored per registration; `cache_provides` in the IR, independent of scope | Runtime expression evaluated once, at registration | PRESERVE |
| `finalizer = ...` | Preserve | Finalizer stored in the leaf's type (`WithFinalizer`); presence in the IR; `NoFinalizer` is zero-sized | Actual finalizer execution | PRESERVE (execution: #61) |
| `extend(...)` | Preserve | Fragment trees nest into one tree; rustc links across fragments | Factory values of every fragment | CHANGED (ADR 0003): a type provided twice and depended on is a compile error instead of "last one wins"; replacement becomes explicit (#62) |
| `instance(value)` | Preserve | Leaf with no dependencies; IR source `Instance` | Actual `value` | PRESERVE |
| Named function factory | Preserve unannotated use | `Instantiator<Deps>` exposes deps, provided type, error | Zero-sized function item | PRESERVE |
| Inline closure | Preserve | Same as named function; parameter types annotated as in Froodi | Closure value | PRESERVE |
| Captured closure | Preserve | Dependency shape from its type | Captured environment stored inline, not boxed | PRESERVE |
| `Instantiator<Deps>` | Prefer preserving core abstraction | Kept: `Provides`, `Error`, `instantiate`; `dependencies()` replaced by parameter metadata | Factory invocation/state | CHANGED (ADR 0001): hand-written impls drop `dependencies()` |
| `DependencyResolver` | Preserve extension semantics where feasible | Custom resolvers are `Resolver` requests: recorded, never linked | Resolver code runs against the container | PRESERVE: same trait shape (`resolve(&Container)`); `Inject`/`InjectTransient` are static edges, not resolvers |
| `Inject<T>` | Preserve | Static shared/scoped edge | Cache/value state | PRESERVE |
| `InjectTransient<T>` | Preserve | Static transient edge | Fresh factory invocation | PRESERVE |
| `Config::cache_provides` | Preserve | Precompile cache policy | Actual cache contents | PRESERVE |
| `Scope` | Preserve | Precompile scope metadata/validation | Scope instances | PRESERVE |
| `Scopes<N>` | Preserve | Precompile hierarchy/accessibility | Runtime selected scope instance | PRESERVE |
| `DefaultScope` | Preserve | Compile known hierarchy | Runtime lifecycle | PRESERVE |
| Custom scopes | Preserve | Compile generic scope metadata if representable | Runtime lifecycle | EXPERIMENT |
| `Context` | Preserve behavior | Possibly precompute access path | Actual context state | PRESERVE |
| Sync factory | Preserve | Static graph + generated execution | Factory state/result | PRESERVE |
| Fallible factory | Preserve | Static graph knows dependency structure | Actual error/result | PRESERVE |
| Async factory | Preserve | Same graph with async execution kind | Future/result | OPEN |
| Sync finalizer | Preserve | Structural ordering may be precomputed | Runtime execution | PRESERVE |
| Async finalizer | Preserve | Structural metadata | Async execution | OPEN |
| thread-safe mode | Preserve | Generated storage must respect feature mode | Runtime synchronization | PRESERVE |
| non-thread-safe mode | Preserve | Avoid hardcoded Arc/Send/Sync | Runtime Rc-style state | PRESERVE |
| `no_std + alloc` direction | Preserve where current Froodi supports it | Compiler tools may use std | Platform runtime | PRESERVE |
| `froodi-auto` | Preserve as optional mechanism | Generate static descriptors/adapters in future | Dynamic/linkme path may remain for runtime mode | OPEN |
| Runtime plugins/modules | Preserve future capability | Not necessarily compile-time | Dynamic registry | RUNTIME ONLY |
| Test overrides | Preserve future capability | Static or runtime depending on composition model | Override selection | OPEN |
| Dynamic registration | Preserve future capability | Explicit boundary from static graph | Runtime registry | RUNTIME ONLY |

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
