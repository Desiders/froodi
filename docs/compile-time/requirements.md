# Compile-Time Froodi — Requirements and Project Constraints

## 1. Project intent

This work is **not** a new dependency injection framework inspired by Froodi.

The experimental implementation may live in new crates for engineering isolation, but the long-term destination is Froodi itself.

The purpose of isolation is to avoid solving two difficult problems simultaneously:

1. designing and validating the new compile-time/static engine;
2. integrating that engine into the current runtime implementation while preserving backward compatibility.

For the current phase:

```text
existing Froodi crates
    current implementation
    API/semantic source of truth
    compatibility target
    benchmark baseline

new experimental crates
    new compile-time/static implementation
    Froodi-compatible API and semantics
    eventual candidate for integration into Froodi
```

Do not redesign the existing Froodi runtime as part of the experimental engine unless a future integration phase explicitly requires it.

## 2. Froodi is the primary source of truth

Before replacing an existing abstraction, inspect the current Froodi source, tests, examples, and benchmarks.

Priority:

```text
1. Froodi
2. Fundle
3. Other Rust DI implementations
4. Other ecosystems
```

External systems may contribute implementation techniques, but they must not dictate Froodi's public API.

## 3. Preserve Froodi terminology and public operations

Important concepts include:

```rust
Container

registry!
async_registry!

Instantiator<Deps>
DependencyResolver

Inject<T>
InjectTransient<T>

instance(...)

Scope
Scopes
DefaultScope

Config
Finalizer
Context
```

Public resolution should continue to use:

```rust
container.get::<T>()
container.get_transient::<T>()
```

Do not introduce `resolve::<T>()` as a replacement API.

Internal generated code may use different terminology.

## 4. Factory definition and registration remain separate

A factory may be declared independently:

```rust
fn make_repository(
    Inject(database): Inject<Database>,
) -> InstantiatorResult<Repository> {
    Ok(Repository::new(database))
}
```

and registered elsewhere:

```rust
registry! {
    scope(App) [
        provide(make_repository),
    ],
}
```

The factory should not have to know its scope, application, registry, or composition root.

Scope, `Config`, and finalizer settings are registration concerns.

The same principle must be considered for named functions, inline closures, captured closures, and `instance(value)`.

Do not make provider annotations mandatory merely because they simplify code generation.

## 5. Compile the graph, not application values

Compile-time DI does not mean every dependency value exists at compile time.

Example:

```rust
let registry = registry! {
    provide(App, instance(config)),
};
```

`config` is a runtime value.

The graph can still potentially know ahead of runtime:

```text
provided type = Config
dependency list = empty
scope = App
registration config
finalizer metadata
```

Likewise, a captured closure is a runtime factory value while its Rust type and dependency shape may still be statically useful.

The architecture must deliberately support:

```text
compile-time topology
+
runtime factory state
```

## 6. Preserve `get` and `get_transient` as distinct semantics

`get<T>()` and `get_transient<T>()` are not merely two names for the same operation.

Likewise `Inject<T>` must retain `get`-like dependency semantics, while `InjectTransient<T>` must retain transient/fresh-construction semantics.

The graph IR must not flatten these into an identical edge.

## 7. Preserve scope and cache as separate dimensions

Do not equate scope with caching.

Froodi exposes configuration equivalent to:

```rust
Config {
    cache_provides: bool,
}
```

A registration may belong to a scope while choosing not to cache the provided result.

The compiled graph and generated execution path must represent scope ownership and cache policy independently.

## 8. Preserve the container lifecycle model

The new engine should remain conceptually compatible with:

```rust
Container::new(...)
Container::new_with_start_scope(...)

container.enter()

container.enter_build()
    .with_scope(...)
    .build()

container.get::<T>()
container.get_transient::<T>()

container.close()
```

Generated component-style internals are acceptable. A Dagger-style generated public component API is not the desired user model.

## 9. Avoid a public type-level dependency graph

Do not require users to manipulate types such as:

```rust
Container<ConfigProvider, DatabaseProvider<ConfigProvider>, ...>
```

Generated internals may use generic specialization, monomorphized functions, generated typed fields, generated resolver functions, and static IDs/tables.

The type-level graph must remain an implementation technique, not the public API.

## 10. Preserve runtime/hybrid capability

Future Froodi must still support runtime/dynamic scenarios such as plugins, runtime-selected modules/registries, test overrides, conditional registration, dynamic auto-registration, and application-provided values.

The static engine must not require the whole dependency graph to be closed forever.

At the same time, runtime capability must not silently weaken compile-time guarantees for registrations intended to be static.

Do not introduce new public markers such as `Dynamic<T>` unless experiments demonstrate that a registration-level distinction cannot express the required boundary.

## 11. Preserve registry composition

The design should aim to retain patterns equivalent to:

```rust
let infrastructure = registry! {
    scope(App) [
        provide(database),
        provide(repository),
    ],
};

let application = registry! {
    extend(infrastructure),

    scope(Request) [
        provide(handler),
    ],
};
```

The architecture must not require one monolithic application component declaration if the existing Froodi registry model can reasonably be preserved.

## 12. Preserve async, finalizers, context and custom scopes architecturally

The first implementation may land synchronous execution first, but the IR and runtime architecture must be checked against `async_registry!`, async instantiators, async finalizers, custom `Scope`/`Scopes`, `Context`, finalization, and parent/child containers.

Do not stabilize a sync-only architecture that would need replacement to support existing Froodi features.

## 13. Preserve thread-safety and `no_std` direction

Do not hardcode the engine around unconditional `Arc` or `std::sync::OnceLock`.

The new runtime should respect Froodi's thread-safe/non-thread-safe and `no_std + alloc` direction where current behavior requires it.

Host-side compiler tooling may use `std`.

## 14. `build.rs` and proc macros are implementation options, not requirements

The design should experimentally compare:

```text
proc macro only
typed macro expansion
proc macro + build.rs
Fundle-like type/compiler encoding
hybrid combinations
```

Cargo staging constraints must be handled honestly.

In particular, do not assume same-crate proc-macro output can simply be consumed by a `build.rs` that runs earlier in the same package build.

## 15. Do not reimplement Rust type resolution

Runtime `TypeId` cannot simply be replaced by type-name strings.

These may denote the same Rust type:

```rust
Foo
crate::Foo
module::Foo
Alias
```

Where possible, split responsibilities:

```text
our compiler:
    graph topology
    registration metadata
    scopes
    dependency modes

rustc:
    actual Rust type compatibility
    aliases
    trait bounds
```

Generated Rust may be used to make rustc validate relationships.

## 16. Benchmark before selecting the most typed architecture

The implementation should compare at least:

```text
A. compiled indexed plan
B. indexed plan + generated typed factory calls
C. generated typed storage
D. hybrid combinations
```

Selection must consider runtime performance, startup cost, clean/incremental build time, binary size, generated code size, diagnostics, implementation complexity, Froodi API compatibility, and hybrid/runtime flexibility.

## 17. Compatibility breaks require explicit justification

Any deliberate incompatibility must document:

```text
existing Froodi behavior
why it cannot reasonably be preserved
alternatives tested
minimal proposed API change
future integration impact
```

“Easier for the new backend” is not sufficient justification.
