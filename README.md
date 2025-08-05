# Froodi - an ergonomic Rust IoC container

[![Crates.io Version](https://img.shields.io/crates/v/froodi)](https://crates.io/crates/froodi)

Froodi is a lightweight, ergonomic Inversion of Control (IoC) container for Rust that helps manage dependencies with clear scoping and lifecycle management in a simple manner.

## Features

- **Scoping**: Any object can have a lifespan for the entire app, a single request, or even more fractionally. 
- **Finalization**: Some dependencies, like database connections, need not only to be created but also carefully released. Many frameworks lack this essential feature.
- **Ergonomic API**: Only a few objects are needed to start using the library.
- **Speed**: Dependency resolving as fast as the speed of light thanks to the Rust
- **Axum integration**: The popular framework for building web applications is supported out of the box
- **Completely safe**: No unsafe code

# Quickstart
```rust
use std::sync::Arc;

use froodi::{
    Container,
    DefaultScope::{App, Request},
    Inject, RegistriesBuilder, instance,
};

#[derive(Clone)]
struct Config {
    // define APP scoped dependency that will be alive throughout the application
    greeting: String,
}

// define REQUEST scoped dependency that will be alive throughout the request
trait UserRepo {
    fn create_user(&self);
}

struct PostgresUserRepo;

impl UserRepo for PostgresUserRepo {
    fn create_user(&self) {
        println!("User created")
    }
}

struct CreateUser<R: UserRepo> {
    // accept the dependency without details about the specific implementation
    repo: Arc<R>, // accept the dependency inside Arc
    config: Arc<Config>,
}

impl<R: UserRepo> CreateUser<R> {
    fn handle(&self) {
        self.repo.create_user();
        println!("{}", self.config.greeting);
    }
}

fn create_container() -> Container {
    Container::new(
        // define the container that stores your dependency factories
        RegistriesBuilder::new()
            .provide(
                instance(Config {
                    greeting: "Hello, user!".to_owned(),
                }),
                App,
            ) // provide APP scoped dependency simply as instance
            .provide(|| Ok(PostgresUserRepo), Request)
            .provide(
                // setup factory for REQUEST scoped dependency that gets other dependencies through Inject
                |Inject(repo): Inject<PostgresUserRepo>, Inject(config): Inject<Config>| {
                    Ok(CreateUser { repo, config })
                },
                Request,
            )
            .add_finalizer(|_dep: Arc<PostgresUserRepo>| println!("repository finalized")),
    )
}

fn handler<R: UserRepo + Send + Sync + 'static>(container: Container) {
    let request_container = container.enter_build().unwrap(); // enter REQUEST scope of container
    let interactor = request_container
        .get::<CreateUser<R>>() // get dependency from REQUEST-scoped container
        .unwrap();

    interactor.handle();
}

fn main() {
    let container = create_container();
    handler::<PostgresUserRepo>(container.clone());
    container.close();
}
```

# Integrations
## Axum
coming soon...

# Contributing

Contributions are welcome!
