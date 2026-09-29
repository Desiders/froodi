//! The end-to-end prototype of issue #64: Froodi-style factories, registry and container usage,
//! with each required extension on top of the baseline.
#![cfg(all(feature = "thread_safe", feature = "async"))]
#![allow(clippy::unnecessary_wraps, reason = "factories return Result by contract")]

use std::sync::{Arc, Mutex};

use froodi_compile::{
    async_impl, async_registry, instance, registry, runtime, Config as Settings, Container, DefaultScope::*, Inject, InjectTransient,
    InstantiatorResult, RuntimeRegistry,
};

#[derive(Clone)]
struct Config {
    url: &'static str,
}
struct Database {
    url: &'static str,
}
struct Repository {
    database: Arc<Database>,
}
struct Service {
    repository: Arc<Repository>,
}

fn make_database(Inject(config): Inject<Config>) -> InstantiatorResult<Database> {
    Ok(Database { url: config.url })
}

fn make_repository(Inject(database): Inject<Database>) -> InstantiatorResult<Repository> {
    Ok(Repository { database })
}

fn make_service(Inject(repository): Inject<Repository>) -> InstantiatorResult<Service> {
    Ok(Service { repository })
}

#[test]
fn baseline() {
    let config = Config { url: "postgres://prod" };
    let registry = registry! {
        provide(App, instance(config)),

        scope(Request) [
            provide(make_database),
            provide(make_repository),
            provide(make_service),
        ],
    };

    let app = Container::new(registry);
    let request = app.clone().enter_build().unwrap();

    let service = request.get::<Service>().unwrap();
    let another = request.get_transient::<Service>().unwrap();

    assert_eq!(service.repository.database.url, "postgres://prod");
    assert!(Arc::ptr_eq(&service.repository, &another.repository));
    assert!(Arc::ptr_eq(&service, &request.get::<Service>().unwrap()));
}

/// Captured closure, `Config { cache_provides: false }`, `extend(...)` and a finalizer.
#[test]
fn captured_closure_uncached_value_extension_and_finalizer() {
    let closed = Arc::new(Mutex::new(Vec::new()));
    let log = closed.clone();
    let prefix = String::from("postgres://");
    let infrastructure = registry! {
        scope(App) [
            provide(move || Ok::<_, froodi_compile::InstantiateErrorKind>(Config { url: if prefix.is_empty() { "" } else { "postgres://captured" } })),
        ],
    };
    let app = Container::new(registry! {
        scope(Request) [
            provide(make_database, finalizer = move |_: Arc<Database>| log.lock().unwrap().push("database")),
            provide(make_repository, config = Settings { cache_provides: false }),
            provide(make_service),
        ],
        extend(infrastructure),
    });

    let request = app.enter_build().unwrap();
    let first = request.get::<Repository>().unwrap();
    let second = request.get::<Repository>().unwrap();

    assert_eq!(first.database.url, "postgres://captured");
    assert!(!Arc::ptr_eq(&first, &second));
    assert!(Arc::ptr_eq(&first.database, &second.database));
    request.close();
    assert_eq!(*closed.lock().unwrap(), vec!["database"]);
}

/// Async factory and finalizer next to sync registrations, in the same graph.
#[tokio::test]
async fn async_factory_and_finalizer() {
    let closed = Arc::new(Mutex::new(Vec::new()));
    let log = closed.clone();
    let app = async_impl::Container::new(async_registry! {
        scope(Request) [
            provide(
                async |Inject(repository): Inject<Repository>| Ok::<_, froodi_compile::InstantiateErrorKind>(Service { repository }),
                finalizer = move |_: Arc<Service>| {
                    let log = log.clone();
                    async move { log.lock().unwrap().push("service") }
                },
            ),
        ],
        extend(registry! {
            provide(App, instance(Config { url: "postgres://async" })),
            scope(Request) [ provide(make_database), provide(make_repository) ],
        }),
    });

    let request = app.enter_build().unwrap();
    assert_eq!(request.get::<Service>().await.unwrap().repository.database.url, "postgres://async");
    request.close().await;
    assert_eq!(*closed.lock().unwrap(), vec!["service"]);
}

/// A plugin registry chosen at runtime, reached from static factories through a declared
/// boundary, and a test override.
#[test]
fn mixed_static_and_runtime_registrations() {
    fn plugins(url: &'static str) -> RuntimeRegistry {
        registry! { provide(App, instance(Config { url })) }.into_runtime()
    }
    let app = Container::new(registry! {
        provide(App, runtime::<Config>()),
        scope(Request) [
            provide(make_database),
            provide(|InjectTransient(database): InjectTransient<Database>| Ok::<_, froodi_compile::InstantiateErrorKind>(Repository { database: Arc::new(database) })),
        ],
        extend(plugins("postgres://plugin")),
    });
    assert_eq!(
        app.clone().enter_build().unwrap().get::<Database>().unwrap().url,
        "postgres://plugin"
    );

    let overridden = Container::new(registry! {
        provide(App, instance(Config { url: "postgres://prod" })),
        scope(Request) [ provide(make_database) ],
        extend(plugins("postgres://test").replacing()),
    });
    assert_eq!(overridden.enter_build().unwrap().get::<Database>().unwrap().url, "postgres://test");
}
