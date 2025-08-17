# Froodi - an ergonomic Rust IoC container

[![Crates.io Version](https://img.shields.io/crates/v/froodi)](https://crates.io/crates/froodi)

Froodi is a lightweight, ergonomic Inversion of Control (IoC) container for Rust that helps manage dependencies with clear scoping and lifecycle management in a simple manner

## Features

- **Scoping**: Any object can have a lifespan for the entire app, a single request, or even more fractionally.
- **Finalization**: Some dependencies, like database connections, need not only to be created but also carefully released. Many frameworks lack this essential feature
- **Ergonomic API**: Only a few objects are needed to start using the library
- **Speed**: Dependency resolving as fast as the speed of light thanks to the Rust
- **Axum integration**: The popular framework for building web applications is supported out of the box
- **Completely safe**: No unsafe code

# Quickstart
```rust
use froodi::{
    Container,
    DefaultScope::{App, Request},
    Inject, InstantiatorResult, RegistriesBuilder, instance,
};
use std::sync::Arc;

// Dependency that will be alive throughout the application
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
    // Dependency without details about the specific implementation.
    // It's inside `Arc` because of caching and finalization features.
    repo: Arc<R>,
}

impl<R: UserRepo> CreateUser<R> {
    fn handle(&self) {
        self.repo.create_user();
    }
}

fn init_container(config: Config) -> Container {
    // We can use functions as instance creators instead of closures
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
    // Enter the container with next non-skipped scope (APP -> REQUEST -> ..., check default scope variants).
    // Don't worry about cloning because it's free.
    let request_container = app_container.clone().enter_build().unwrap();

    // Get REQUEST-scoped dependency from REQUEST-scoped container
    let interactor = request_container.get::<CreateUser<PostgresUserRepo>>().unwrap();
    interactor.handle();

    // Get APP-scoped dependency from REQUEST-scoped container.
    // We can use dependencies from previous containers.
    let _config = request_container.get::<Config>().unwrap();

    // We need to close containers after usage of them.
    // Currently, it's not necessary, but we usually need to call finalizers of cached dependencies when we close. Check finalizer example.
    request_container.close();
    app_container.close();
}
```

# Integrations
## Axum
coming soon...

# Contributing

Contributions are welcome!
