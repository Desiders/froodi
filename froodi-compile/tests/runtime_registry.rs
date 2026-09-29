//! Runtime registries: type-erased fragments linked by binding key when the container is built
//! (issues #56, #59 level A, #62).
#![allow(clippy::unnecessary_wraps, reason = "factories return Result by contract")]

use froodi_compile::{
    ir::Diagnostic, registry, Container, DefaultScope::*, Inject, InjectTransient, InstantiateErrorKind, RuntimeRegistry,
};

#[derive(Clone)]
struct Config(&'static str);
struct Database(&'static str);
struct Seed;

fn make_config() -> Result<Config, InstantiateErrorKind> {
    Ok(Config("runtime"))
}

fn make_database(Inject(config): Inject<Config>, InjectTransient(_seed): InjectTransient<Seed>) -> Result<Database, InstantiateErrorKind> {
    Ok(Database(config.0))
}

/// A fragment built in a function: its type is erased, so it can be named and returned.
fn infrastructure() -> RuntimeRegistry {
    registry! {
        scope(App) [
            provide(make_config),
            provide(|| Ok::<_, InstantiateErrorKind>(Seed)),
            provide(make_database),
        ],
    }
    .into_runtime()
}

#[test]
fn resolves_a_runtime_registry_by_key() {
    let container = Container::new(registry! {
        extend(infrastructure()),
    });

    assert_eq!(container.get::<Database>().unwrap().0, "runtime");
    assert!(froodi_compile::thread_safety::RcThreadSafety::ptr_eq(
        &container.get::<Config>().unwrap(),
        &container.get::<Config>().unwrap()
    ));
}

#[test]
fn reports_a_missing_binding_of_a_runtime_registry_when_the_container_is_built() {
    let result = Container::try_new(registry! {
        extend(registry! { scope(App) [ provide(make_config), provide(make_database) ] }.into_runtime()),
    });

    let diagnostics = result.err().expect("Seed is missing");
    assert!(matches!(diagnostics.0.as_slice(), [Diagnostic::MissingBinding { missing, .. }] if missing.ends_with("Seed")));
}

#[test]
fn a_runtime_registration_depends_on_a_static_one() {
    let plugins = registry! {
        scope(App) [
            provide(|| Ok::<_, InstantiateErrorKind>(Seed)),
            provide(make_database),
        ],
    }
    .into_runtime();
    let container = Container::new(registry! {
        scope(App) [
            provide(make_config),
        ],
        extend(plugins),
    });

    assert_eq!(container.get::<Database>().unwrap().0, "runtime");
    assert!(froodi_compile::thread_safety::RcThreadSafety::ptr_eq(
        &container.get::<Config>().unwrap(),
        &container.get::<Config>().unwrap()
    ));
}

#[test]
fn reports_a_cycle_through_runtime_registrations_with_its_path() {
    struct A;
    struct B;
    let result = Container::try_new(registry! {
        extend(registry! {
            scope(App) [
                provide(|Inject(_b): Inject<B>| Ok::<_, InstantiateErrorKind>(A)),
                provide(|Inject(_a): Inject<A>| Ok::<_, InstantiateErrorKind>(B)),
            ],
        }
        .into_runtime()),
    });

    let diagnostics = result.err().expect("the cycle is reported");
    assert!(diagnostics.to_string().starts_with("error: dependency cycle"), "{diagnostics}");
}

struct Plugin(&'static str);
struct Host(froodi_compile::thread_safety::RcThreadSafety<Plugin>);

fn make_host(Inject(plugin): Inject<Plugin>) -> Result<Host, InstantiateErrorKind> {
    Ok(Host(plugin))
}

#[test]
fn a_static_registration_reaches_a_runtime_one_through_a_declared_boundary() {
    let plugins = registry! {
        scope(App) [ provide(|| Ok::<_, InstantiateErrorKind>(Plugin("loaded"))) ],
    }
    .into_runtime();
    let container = Container::new(registry! {
        scope(App) [
            provide(froodi_compile::runtime::<Plugin>()),
            provide(make_host),
        ],
        extend(plugins),
    });

    let host = container.get::<Host>().unwrap();
    assert_eq!(host.0 .0, "loaded");
    assert!(froodi_compile::thread_safety::RcThreadSafety::ptr_eq(
        &host.0,
        &container.get::<Plugin>().unwrap()
    ));
}

#[test]
fn a_declared_boundary_without_a_runtime_provider_fails_when_the_container_is_built() {
    let result = Container::try_new(registry! {
        scope(App) [
            provide(froodi_compile::runtime::<Plugin>()),
            provide(make_host),
        ],
    });

    let diagnostics = result.err().expect("nothing provides Plugin at runtime");
    assert!(matches!(diagnostics.0.as_slice(), [Diagnostic::MissingBinding { missing, .. }] if missing.ends_with("Plugin")));
}
