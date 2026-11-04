# `froodi`

[![Crates.io][crates-badge]][crates-url]
[![Docs.rs][docs-badge]][docs-url]
[![License][license-badge]][license_apache]
[![Telegram][telegram-badge]][telegram]

Froodi is a lightweight, ergonomic Inversion of Control (IoC) container for Rust that helps manage dependencies with clear scoping and lifecycle management in a simple manner.

📚 [Documentation][docs-url]

### Purpose

`froodi` is for applications where object wiring has become repetitive enough that you want a container,
but you still want lifetimes to stay explicit.

It focuses on a small set of DI problems:

- how to register factories
- how to express object lifetime using scopes
- how to reuse scoped dependencies safely
- how to create transient values when you need a fresh one
- how to clean up resolved dependencies with finalizers
- how to plug the container into framework request handling

### Key features

- **Scopes**. Built-in scopes set where a dependency can be resolved and how long a cached value lives.
- **Thread safety**. Thread safety is enabled by default. You can disable it to use `Rc`-based internals instead of `Arc` and remove `Send` / `Sync` requirements.
- **Finalizers**. Registrations can run cleanup when a scope closes after `get` resolves a value.
- **Sync and async support**. The crate supports both sync and async factories and containers.
- **Reusable registrations**. Syntax fragments and typed registry values compose registrations; erased `Registry` values mark a runtime boundary.
- **Registration checks**. Typed registration links known dependencies at compile time. Closed graphs can also reject cycles and incompatible static scopes when built.
- **Struct construction**. `#[derive(froodi::Construct)]` wires struct fields as dependencies; `provide::<T>(scope)` registers the type.
- **Auto-registration**. `froodi-auto` can collect providers declared with macros.
- **Framework integrations**. Optional features support `axum`, `dptree`, `telers`, and `ruststream`.

## Quickstart

Add Froodi to your application:

```toml
[dependencies]
froodi = "1.0.0-beta.20"
```

This example registers an API client, a connection, a data access implementation,
and a service:

```rust
use froodi::{
    Construct, Container,
    DefaultScope::{App, Request},
    Inject, boxed, registry,
};
use std::sync::Arc;

#[derive(Construct)]
struct APIClient;

#[derive(Construct)]
struct Conn;

trait UserDao: Send + Sync {}

struct MemoryUserDao {
    conn: Arc<Conn>,
}

impl UserDao for MemoryUserDao {}

#[derive(Construct)]
struct Service {
    #[di(inject)]
    client: Arc<APIClient>,
    #[di(inject_transient)]
    dao: Box<dyn UserDao>,
}

fn main() {
    let registry = registry! {
        provide::<APIClient>(App),
        scope(Request) [
            provide::<Conn>(),
            provide(|Inject(conn)| Ok(boxed!(MemoryUserDao { conn }; UserDao))),
            provide::<Service>(),
        ]
    };
    let container = Container::new_with_start_scope(registry, Request);

    let _service = container.get_transient::<Service>().unwrap();

    container.close();
}
```

`provide::<T>(scope)` registers a type implementing `Construct`; the derive
supplies that implementation in this example. Inside a scope block, use `provide::<T>()`.
`#[di(inject)]` uses a scoped, shared dependency; `#[di(inject_transient)]`
creates a fresh value. The example starts directly in `Request` to stay small.
In an application, you can start in `App` with `Container::new(registry)` and
enter a request scope when handling a request.

See the runnable [registration example][examples/registration]. The
[fragments example][examples/fragments] splits the same registrations across
modules; the [async example][examples/async_registration] mixes sync and async
factories in one `registry!`.

With the `async` feature, add `#[di(async)]` below `#[derive(Construct)]` to
resolve a struct's fields asynchronously. The registration forms stay the same;
its dependencies can use sync or async factories.

## Concepts

### Dependency

A dependency is simply a value constructed by the container.
Factories can depend on other values, and `froodi` resolves those dependencies recursively.

### Scope

A scope determines where a dependency is accessible. Cached values live until
their owning scope closes; transient values are constructed afresh.

The built-in default scope chain is:

`Runtime -> App -> Session -> Request -> Action -> Step`

`Runtime` and `Session` are optional by default.
That means:

- `Container::new(...)` usually starts from `App`
- `container.enter_build()` usually goes from `App` to `Request`

If you want one of the optional scopes explicitly:

```rust
use froodi::{Container, DefaultScope::{Request, Runtime, Session}, registry};

fn main() {
    let runtime_container = Container::new_with_start_scope(registry! {
        scope(Runtime) [
            provide(|| Ok(())),
        ],
        scope(Session) [
            provide(|| Ok(((), ()))),
        ],
        scope(Request) [
            provide(|| Ok(((), (), ()))),
        ],
    }, Runtime);

    let session_container = runtime_container.clone().enter().with_scope(Session).build().unwrap();
    session_container.close();
    runtime_container.close();
}
```

### Container

The container holds resolved scoped dependencies and is used to access them.

- `get::<T>()` resolves a dependency according to its scope and cache settings
- `get_transient::<T>()` constructs a fresh value instead of using its cache or
  a `Context` value, and does not schedule its finalizer
- `enter_build()` creates the next child scope
- `close()` runs finalizers for resolved dependencies in that scope

If a child container was created by skipping optional parent scopes, closing the child also closes those skipped parents.
For example, a request container created from an app container also closes the skipped `Session` scope.

### Registry

The registry defines how dependencies are constructed.

The main registration forms are:

- `provide(scope, factory)`
- `provide::<T>(scope)` for a type implementing `Construct` (usually with the derive)
- `scope(ScopeName) [ provide(factory), provide::<T>(), ... ]`
- `extend(other_registry)`
- `extend_fragment(client::registrations!(), service::registrations!())` for reusable syntax
- `instance(value)` for values created outside the container

Use `#[froodi::fragment(name)]` at module scope to save a registry template.
Fragments contribute registrations to the final typed graph, so dependencies
can cross fragment boundaries. They can also accept expression arguments.

```rust
use froodi::{Construct, Container, registry};

#[derive(Construct)]
struct APIClient;

#[froodi::fragment(client)]
registry! {
    use froodi::DefaultScope::App;
    use crate::APIClient;

    provide::<APIClient>(App),
}

fn main() {
    let container = Container::new(registry! {
        extend_fragment(client!()),
    });
    let _client = container.get::<APIClient>().unwrap();
    container.close();
}
```

`extend_fragment` includes reusable syntax; `extend` accepts registry values.
Use `.into_registry()` on a final composition when you need an erased `Registry`.
See the [fragments example][examples/fragments] for composition across modules.

### Finalizer

A finalizer is cleanup logic attached to a registration. It runs when the
owning scope closes after a value was resolved through `get`. Transient
resolution does not schedule a finalizer.

```rust
use froodi::{
    Construct, Container,
    DefaultScope::Request,
    registry,
};

#[derive(Construct)]
struct Conn;

fn main() {
    let container = Container::new_with_start_scope(registry! {
        scope(Request) [
            provide::<Conn>(finalizer = |_dep| println!("Conn finalized")),
        ]
    }, Request);

    let _conn = container.get::<Conn>().unwrap();
    container.close();
}
```

### Trait objects

Use `boxed!` when a factory should expose a trait object. In the quickstart,
`boxed!(MemoryUserDao { conn }; UserDao)` provides `Box<dyn UserDao>`, which
`Service` receives through `#[di(inject_transient)]`.

## Features

Common feature combinations:

```toml
[dependencies]
froodi = { version = "1.0.0-beta.20", features = ["async", "axum"] } # choose the flags you need
froodi-auto = { version = "1", features = ["async"] }
```

Important feature flags:

- `thread_safe` (enabled by default)
- `async`
- `axum`
- `http2-axum`
- `dptree`
- `telers` (requires Rust 1.88 or newer through telers)
- `ruststream`

Disable default features if you want to turn off `thread_safe`.

### Registration checks

When a typed registry is consumed, `Inject<T>` and `InjectTransient<T>` link to
providers by type. A missing or ambiguous provider is a compilation error.
Closed typed graphs can also reject cycles when the container constructor is
instantiated during `cargo build` or `cargo test`.

Declare values supplied later through `Context` or an erased registry with
`declare::<T>()`. Use `InjectCustom<T>` when a custom resolver performs its own
lookups. These are runtime boundaries; the typed linker cannot inspect the
values they will supply. Distinct `StaticScope` types allow compile-time checks
for eligible edges in a closed typed graph. `DefaultScope` variants and other
runtime-valued scopes keep runtime checks.

Use `TypedContainer` when you also want unknown `get::<T>()` requests to fail at
compile time. For more detail, see the API documentation for [`registry!`][docs-url].

With the `async` feature, the same `registry!` accepts sync and async factories
together. `async_impl::Container` resolves either kind; `Container` resolves only
sync providers, even when unrelated async providers are registered.
Sync factories can depend only on sync providers; async factories can use either.

Use `.into_registry()` when a first-class `Registry` value is needed. It erases
the typed provider graph, so further composition keeps runtime validation for
relationships that cannot be proven from the final typed syntax. `Registry`
can hold sync, async, or mixed registrations.

## Examples

- [Registration][examples/registration]. Basic synchronous container setup
- [Fragments][examples/fragments]. Compose API client, data access, and service registrations across modules
- [Async registration][examples/async_registration]. Sync and async factories in one registry
- [Finalization][examples/finalization]. Scoped cleanup with synchronous finalizers
- [Context][examples/context]. Supply request values through `Context`
- [Axum integration][examples/axum_integration]. Request injection in `axum`
- [Dptree integration][examples/dptree_integration]. Endpoint injection in `dptree`
- [Telers integration][examples/telers_integration]. Handler injection in `telers`
- [RustStream integration][examples/ruststream_integration]. Handler injection in `ruststream`

Browse the full [examples directory][examples].

## Community

- 🇺🇸 🇷🇺 [@froodi_di][telegram]

## Contributing

Contributions are welcome.

## License

[Apache License, Version 2.0][license_apache]

[examples]: https://github.com/Desiders/froodi/tree/master/examples
[examples/registration]: https://github.com/Desiders/froodi/tree/master/examples/registration
[examples/fragments]: https://github.com/Desiders/froodi/tree/master/examples/fragments
[examples/async_registration]: https://github.com/Desiders/froodi/tree/master/examples/async_registration
[examples/finalization]: https://github.com/Desiders/froodi/tree/master/examples/finalization
[examples/context]: https://github.com/Desiders/froodi/tree/master/examples/context
[examples/axum_integration]: https://github.com/Desiders/froodi/tree/master/examples/axum_integration
[examples/dptree_integration]: https://github.com/Desiders/froodi/tree/master/examples/dptree_integration
[examples/telers_integration]: https://github.com/Desiders/froodi/tree/master/examples/telers_integration
[examples/ruststream_integration]: https://github.com/Desiders/froodi/tree/master/examples/ruststream_integration

[docs-badge]: https://docs.rs/froodi/badge.svg
[docs-url]: https://docs.rs/froodi
[crates-badge]: https://img.shields.io/crates/v/froodi.svg
[crates-url]: https://crates.io/crates/froodi
[license-badge]: https://img.shields.io/github/license/Desiders/froodi
[license_apache]: https://github.com/Desiders/froodi/blob/master/froodi/LICENSE
[telegram-badge]: https://img.shields.io/badge/%F0%9F%92%AC-Telegram-blue
[telegram]: https://t.me/froodi_di
