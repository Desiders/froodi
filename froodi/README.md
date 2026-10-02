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

- **Scopes**. Built-in scopes let dependencies live for the whole application, a request, or even shorter units.
- **Thread safety**. Thread safety is enabled by default. You can disable it to use `Rc`-based internals instead of `Arc` and remove `Send` / `Sync` requirements.
- **Finalizers**. Dependencies can register cleanup logic that runs when a scope is closed.
- **Sync and async support**. The crate supports both sync and async factories and containers.
- **Modular registries**. Registries can be split and extended instead of building one large registration block.
- **Auto-registration**. `froodi-auto` can collect providers declared with macros.
- **Framework integrations**. `axum`, `dptree`, `telers`, and `ruststream` are supported out of the box.

## Quickstart

1. **Install the crate.**

```toml
[dependencies]
froodi = "1.0.0-beta.18"
```

2. **Define your types.**

In this example:

- `Config` lives for the whole application
- `GreetingService` is created per request
- `WelcomeHandler` depends on the greeter and is resolved as a transient value

```rust
use std::sync::Arc;

trait Greeter: Send + Sync {
    fn greet(&self, name: &str) -> String;
}

#[derive(Clone)]
struct Config {
    greeting: String,
}

struct GreetingService {
    greeting: String,
}

impl Greeter for GreetingService {
    fn greet(&self, name: &str) -> String {
        format!("{}, {name}!", self.greeting)
    }
}

struct WelcomeHandler {
    greeter: Arc<Box<dyn Greeter>>,
}

impl WelcomeHandler {
    fn handle(&self, name: &str) {
        println!("{}", self.greeter.greet(name));
    }
}
```

3. **Register factories in a registry.**

```rust
use froodi::{
    Container,
    DefaultScope::{App, Request},
    Inject, boxed, instance, registry,
};

fn build_container(cfg: Config) -> Container {
    Container::new(registry! {
        provide(App, instance(cfg)),
        scope(Request) [
            provide(|Inject(cfg): Inject<Config>| {
                Ok(boxed!(GreetingService { greeting: cfg.greeting.clone() }; Greeter))
            }),
            provide(|Inject(greeter)| {
                Ok(WelcomeHandler { greeter })
            }),
        ],
    })
}
```

4. **Create a container and enter the next scope.**

`Container::new(...)` starts at the first non-optional default scope, which is usually `App`.
`enter_build()` moves to the next non-optional child scope, which is usually `Request`.

```rust
let app_container = build_container(Config {
    greeting: "Hello".to_owned(),
});

let request_container = app_container.clone().enter_build().unwrap();
```

5. **Resolve dependencies.**

Use `get::<T>()` for scoped shared dependencies and `get_transient::<T>()` for fresh values.

```rust
let handler = request_container.get_transient::<WelcomeHandler>().unwrap();
handler.handle("froodi");

let config = request_container.get::<Config>().unwrap();
assert_eq!(config.greeting, "Hello");
```

6. **Close containers when done.**

```rust
request_container.close();
app_container.close();
```

<details>
<summary>Full example</summary>

```rust
use froodi::{
    Container,
    DefaultScope::{App, Request},
    Inject, boxed, instance, registry,
};
use std::sync::Arc;

trait Greeter: Send + Sync {
    fn greet(&self, name: &str) -> String;
}

#[derive(Clone)]
struct Config {
    greeting: String,
}

struct GreetingService {
    greeting: String,
}

impl Greeter for GreetingService {
    fn greet(&self, name: &str) -> String {
        format!("{}, {name}!", self.greeting)
    }
}

struct WelcomeHandler {
    greeter: Arc<Box<dyn Greeter>>,
}

impl WelcomeHandler {
    fn handle(&self, name: &str) {
        println!("{}", self.greeter.greet(name));
    }
}

fn build_container(cfg: Config) -> Container {
    Container::new(registry! {
        provide(App, instance(cfg)),
        scope(Request) [
            provide(|Inject(cfg): Inject<Config>| {
                Ok(boxed!(GreetingService { greeting: cfg.greeting.clone() }; Greeter))
            }),
            provide(|Inject(greeter)| {
                Ok(WelcomeHandler { greeter })
            }),
        ],
    })
}

fn main() {
    let app_container = build_container(Config {
        greeting: "Hello".to_owned(),
    });

    let request_container = app_container.clone().enter_build().unwrap();

    let handler = request_container.get_transient::<WelcomeHandler>().unwrap();
    handler.handle("froodi");

    let config = request_container.get::<Config>().unwrap();
    assert_eq!(config.greeting, "Hello");

    request_container.close();
    app_container.close();
}
```

</details>

7. **(Optional) Add async support or framework integration.**

- For async containers and factories, see [async registration][examples/async_registration]
- For `froodi-auto`, see [sync auto registration][examples/sync_auto_registration] and [async auto registration][examples/async_auto_registration]
- For framework integration, see [axum][examples/axum_integration], [dptree][examples/dptree_integration], [telers][examples/telers_integration], and [ruststream][examples/ruststream_integration]

## Concepts

### Dependency

A dependency is simply a value constructed by the container.
Factories can depend on other values, and `froodi` resolves those dependencies recursively.

### Scope

A scope describes how long a dependency lives.

The built-in default scope chain is:

`Runtime -> App -> Session -> Request -> Action -> Step`

`Runtime` and `Session` are optional by default.
That means:

- `Container::new(...)` usually starts from `App`
- `container.enter_build()` usually goes from `App` to `Request`

If you want one of the optional scopes explicitly:

```rust
use froodi::{Container, DefaultScope::{Request, Runtime, Session}, registry};

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
```

### Container

The container holds resolved scoped dependencies and is used to access them.

- `get::<T>()` returns a scoped shared dependency
- `get_transient::<T>()` creates a fresh value
- `enter_build()` creates the next child scope
- `close()` runs finalizers for resolved dependencies in that scope

If a child container was created by skipping optional parent scopes, closing the child also closes those skipped parents.
For example, a request container created from an app container also closes the skipped `Session` scope.

### Registry

The registry defines how dependencies are constructed.

The main registration forms are:

- `provide(scope, factory)`
- `scope(ScopeName) [ provide(factory), ... ]`
- `extend(other_registry)`
- `instance(value)` for values created outside the container

### Finalizer

A finalizer is cleanup logic attached to a registered dependency.
It is executed when the owning scope is closed.

```rust
use froodi::{
    Container,
    DefaultScope::App,
    instance, registry,
};

#[derive(Clone)]
struct AppState;

let container = Container::new_with_start_scope(
    registry! {
        provide(App, instance(AppState), finalizer = |_dep| println!("AppState finalized")),
    },
    App,
);

let _state = container.get::<AppState>().unwrap();
container.close();
```

### Trait objects

Use `boxed!` when the provided type should be exposed as a trait object.

```rust
use froodi::{Inject, boxed, registry};
use froodi::DefaultScope::Request;

trait Greeter {
    fn greet(&self) -> &'static str;
}

struct GreetingService;

impl Greeter for GreetingService {
    fn greet(&self) -> &'static str {
        "hello"
    }
}

struct Handler {
    greeter: std::sync::Arc<Box<dyn Greeter>>,
}

let registry = registry! {
    scope(Request) [
        provide(|| Ok(boxed!(GreetingService; Greeter))),
        provide(|Inject(greeter)| Ok(Handler { greeter })),
    ],
};
```

## Features

Common feature combinations:

```toml
[dependencies]
froodi = { version = "1.0.0-beta.18", features = ["async", "axum"] } # choose the flags you need
froodi-auto = { version = "1", features = ["async"] }
```

Important feature flags:

- `thread_safe` (enabled by default)
- `async`
- `compiled` (opt-in typed registries through the existing Container)
- `axum`
- `http2-axum`
- `dptree`
- `telers`
- `ruststream`

Disable default features if you want to turn off `thread_safe`.

### Compiled registrations

Enable `compiled` and use `froodi::compiled_registry!` or, with `async`,
`froodi::compiled_async_registry!`.
Ordinary Froodi macros remain dynamic. The existing `Container`, injection modes
and lifecycle are shared. Custom parameters use `froodi::RuntimeDependency<T>`;
opaque boundaries use `froodi::runtime::<T>()` and `froodi::context::<T>()`.

For compile-time checks on public lookups, use `TypedContainer` with concrete typed
registrations and import `TypedContainerExt`:

```rust
use froodi::{compiled_registry, instance, TypedContainer, TypedContainerExt as _, DefaultScope::App};

let container = TypedContainer::new(compiled_registry! { provide(App, instance(7u32)) });
assert_eq!(*container.get::<u32>().unwrap(), 7);
// container.get::<String>() fails to compile: no String provider.
```

The async equivalents live in `froodi::async_impl`. Cloning and entering child
scopes retain provider proofs. `into_container()` discards them for ordinary
lookups and integrations. Scope accessibility and construction errors remain
runtime concerns. Native fragments and runtime/Context provider declarations use
the ordinary `Container`. Typed lookups delegate to its existing lookup path;
compiled instantiator dependencies continue using indexed edges.

## Examples

- [Sync registration][examples/sync_registration]. Basic sync container setup
- [Sync compiled registration][examples/sync_compiled_registration]. Typed registration with the existing container
- [Async registration][examples/async_registration]. Basic async container setup
- [Sync finalization][examples/sync_finalization]. Scoped cleanup with sync finalizers
- [Async finalization][examples/async_finalization]. Scoped cleanup with async finalizers
- [Sync auto registration][examples/sync_auto_registration]. Sync auto-registration with `froodi-auto`
- [Async auto registration][examples/async_auto_registration]. Async auto-registration with `froodi-auto`
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
[examples/sync_registration]: https://github.com/Desiders/froodi/tree/master/examples/sync_registration
[examples/sync_compiled_registration]: https://github.com/Desiders/froodi/tree/master/examples/sync_compiled_registration
[examples/async_registration]: https://github.com/Desiders/froodi/tree/master/examples/async_registration
[examples/sync_auto_registration]: https://github.com/Desiders/froodi/tree/master/examples/sync_auto_registration
[examples/async_auto_registration]: https://github.com/Desiders/froodi/tree/master/examples/async_auto_registration
[examples/sync_finalization]: https://github.com/Desiders/froodi/tree/master/examples/sync_finalization
[examples/async_finalization]: https://github.com/Desiders/froodi/tree/master/examples/async_finalization
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
