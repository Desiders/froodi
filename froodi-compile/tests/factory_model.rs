//! Evidence for the factory model (issue #55): what code constrained by `F: Instantiator<Deps>`
//! knows statically, and what stays a runtime value.

#![allow(clippy::unnecessary_wraps, reason = "factories return Result by contract")]

use core::any::{type_name, TypeId};
use core::mem::size_of_val;

use froodi_compile::{instance, registry, DefaultScope::*, Inject, InjectTransient, InstantiateErrorKind, Instantiator};

#[derive(Clone)]
struct Config;
struct Database;

fn make_database(Inject(_config): Inject<Config>, InjectTransient(_seed): InjectTransient<u64>) -> Result<Database, InstantiateErrorKind> {
    Ok(Database)
}

/// Everything generic code learns from the bound alone, without calling the factory.
fn shape<F: Instantiator<Deps>, Deps: 'static>(_factory: &F) -> (TypeId, TypeId, &'static str) {
    (TypeId::of::<Deps>(), TypeId::of::<F::Provides>(), type_name::<F::Error>())
}

#[test]
fn named_function_exposes_dependencies_provided_type_and_error() {
    let (deps, provides, error) = shape(&make_database);

    assert_eq!(deps, TypeId::of::<(Inject<Config>, InjectTransient<u64>)>());
    assert_eq!(provides, TypeId::of::<Database>());
    assert_eq!(error, type_name::<InstantiateErrorKind>());
}

#[test]
fn inline_closure_exposes_the_same_shape() {
    let factory = |Inject(_config): Inject<Config>| Ok::<_, InstantiateErrorKind>(Database);

    let (deps, provides, _) = shape(&factory);

    assert_eq!(deps, TypeId::of::<(Inject<Config>,)>());
    assert_eq!(provides, TypeId::of::<Database>());
}

#[test]
fn captured_closure_keeps_its_environment_as_a_runtime_value() {
    let url = String::from("postgres://localhost");
    let factory = move || Ok::<_, InstantiateErrorKind>(url.clone());

    let (deps, provides, _) = shape(&factory);

    assert_eq!(deps, TypeId::of::<()>());
    assert_eq!(provides, TypeId::of::<String>());
    assert_eq!(size_of_val(&factory), size_of_val(&String::new()));
}

#[test]
fn instance_is_a_factory_without_dependencies() {
    let (deps, provides, _) = shape(&instance(Config));

    assert_eq!(deps, TypeId::of::<()>());
    assert_eq!(provides, TypeId::of::<Config>());
}

#[test]
fn registry_stores_factory_values_without_boxing() {
    // Two registries that differ only in the factory's captured environment differ in size by
    // exactly that environment. Erasing factories to `Box<dyn Instantiator>` would make both the
    // same size.
    let seed: u64 = 7;
    let captured = registry! {
        scope(App) [
            provide(make_database),
            provide(move || Ok::<_, InstantiateErrorKind>(seed)),
            provide(instance(Config)),
        ],
    };
    let unit = registry! {
        scope(App) [
            provide(make_database),
            provide(|| Ok::<_, InstantiateErrorKind>(7_u64)),
            provide(instance(Config)),
        ],
    };

    assert_eq!(size_of_val(&captured) - size_of_val(&unit), size_of_val(&seed));
}
