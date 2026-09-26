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
| `registry!` | Preserve syntax aggressively | Candidate compiler frontend/composition root | Factory values | EXPERIMENT |
| `async_registry!` | Preserve | Same graph model with async execution metadata | Futures/runtime values | OPEN |
| `provide(scope, factory)` | Preserve | Compile registration metadata | Concrete factory value | PRESERVE |
| `scope(scope) [ provide(...) ]` | Preserve | Compile scope metadata | Runtime scope state | PRESERVE |
| `config = ...` | Preserve | Compile config metadata where possible | Runtime expression/value if needed | PRESERVE |
| `finalizer = ...` | Preserve | Attach finalizer metadata to registration | Actual finalizer execution | PRESERVE |
| `extend(...)` | Preserve | Compile/merge static registry fragments where possible | Dynamic fragments if any | EXPERIMENT |
| `instance(value)` | Preserve | Static topology: provides T, no DI deps | Actual `value` | PRESERVE |
| Named function factory | Preserve unannotated use | Infer through typed registration/`Instantiator<Deps>` | Function value | PRESERVE |
| Inline closure | Preserve | Typed registration/macro frontend | Closure value | EXPERIMENT |
| Captured closure | Preserve | Compile dependency shape if possible | Captured environment | EXPERIMENT |
| `Instantiator<Deps>` | Prefer preserving core abstraction | Potential bridge from typed factory to graph metadata | Factory invocation/state | EXPERIMENT |
| `DependencyResolver` | Preserve extension semantics where feasible | Dependency mode may become graph metadata | Custom resolver runtime work if required | OPEN |
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
