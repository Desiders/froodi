# Froodi - an ergonomic Rust IoC container

[![Crates.io][crates-badge]][crates-url]

Froodi is a lightweight, ergonomic Inversion of Control (IoC) container for Rust that helps manage dependencies with clear scoping and lifecycle management in a simple manner

## Features

- **Scoping**: Any object can have a lifespan for the entire app, a single request, or even more fractionally.
- **Finalization**: Some dependencies, like database connections, need not only to be created but also carefully released. Many frameworks lack this essential feature
- **Ergonomic**: Simple API
- **Speed**: Dependency resolving as fast as the speed of light thanks to the Rust
- **Integration**: The popular frameworks for building web applications is supported out of the box
- **Safe**: 100% safe Rust (no unsafe used)

# Quickstart
```rust
use froodi::{
    Container,
    DefaultScope::{App, Request},
    Inject, InstantiatorResult, RegistriesBuilder, instance,
};
use std::sync::Arc;

#[derive(Default, Clone)]
struct Config {
    _host: &'static str,
    _port: i16,
    _user: &'static str,
    _password: &'static str,
    _db: &'static str,
}

trait UserRepo {
    fn create_user(&self);
}

struct PostgresUserRepo;

impl UserRepo for PostgresUserRepo {
    fn create_user(&self) {
        todo!()
    }
}

struct CreateUser<R> {
    repo: Arc<R>,
}

impl<R: UserRepo> CreateUser<R> {
    fn handle(&self) {
        self.repo.create_user();
    }
}

fn init_container(config: Config) -> Container {
    fn create_user<R>(Inject(repo): Inject<R>) -> InstantiatorResult<CreateUser<R>> {
        Ok(CreateUser { repo })
    }

    let registry = RegistriesBuilder::new()
        .provide(instance(config), App)
        .provide(|_config: Inject<Config>| Ok(PostgresUserRepo), Request)
        .provide(create_user::<PostgresUserRepo>, Request);
    Container::new(registry)
}

fn main() {
    let app_container = init_container(Config::default());
    let request_container = app_container.clone().enter_build().unwrap();

    let interactor = request_container.get::<CreateUser<PostgresUserRepo>>().unwrap();
    interactor.handle();

    let _config = request_container.get::<Config>().unwrap();

    request_container.close();
    app_container.close();
}
```

## Examples
 - [Sync provide][examples/sync_provide]. This example shows how to provide sync dependencies.
 - [Async provide][examples/async_provide]. This example shows how to provide async sync dependencies.
 - [Sync finalizer][examples/sync_finalizer]. This example shows how to add sync finalizers.
 - [Async finalizer][examples/async_finalizer]. This example shows how to add async finalizers.

You may consider checking out [this directory][examples] for examples.

# Integrations
## Axum
coming soon...

# Contributing

Contributions are welcome!

## License
[Apache License, Version 2.0][license_apache]

[examples]: https://github.com/Desiders/froodi/tree/master/examples
[examples/sync_provide]: https://github.com/Desiders/froodi/tree/master/examples/sync_provide
[examples/async_provide]: https://github.com/Desiders/froodi/tree/master/examples/async_provide
[examples/sync_finalizer]: https://github.com/Desiders/froodi/tree/master/examples/sync_finalizer
[examples/async_finalizer]: https://github.com/Desiders/froodi/tree/master/examples/async_finalizer

[license_apache]: https://github.com/Desiders/froodi/blob/master/froodi/LICENSE
[docs]: https://docs.rs/froodi
[crates-badge]: https://img.shields.io/crates/v/froodi.svg
[crates-url]: https://crates.io/crates/froodi
