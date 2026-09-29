//! The registration IR a registry produces (issue #57).

#![allow(clippy::unnecessary_wraps, reason = "factories return Result by contract")]

use core::any::{type_name, TypeId};

use froodi_compile::{
    instance,
    ir::{DependencyRequest, ExecutionKind, RequestMode, ScopeKey, Target, ValueSource},
    registry,
    DefaultScope::*,
    Inject, InjectTransient, InstantiateErrorKind,
};

#[derive(Clone)]
struct Config;
struct Database;

fn make_config() -> Result<Config, InstantiateErrorKind> {
    Ok(Config)
}

fn make_database(Inject(_config): Inject<Config>) -> Result<Database, InstantiateErrorKind> {
    Ok(Database)
}

const APP: ScopeKey = ScopeKey {
    priority: 1,
    name: "app",
    skipped_by_default: false,
};

#[test]
fn describes_each_registration_in_declaration_order() {
    let graph = registry! {
        scope(App) [
            provide(make_config),
            provide(make_database),
        ],
    }
    .graph();

    let keys: Vec<_> = graph.registrations.iter().map(|registration| registration.key).collect();
    assert_eq!(keys, vec![TypeId::of::<Config>(), TypeId::of::<Database>()]);

    let database = &graph.registrations[1];
    assert_eq!(database.type_name, type_name::<Database>());
    assert_eq!(database.scope, APP);
    assert_eq!(
        database.requests,
        vec![DependencyRequest {
            target: Target::Key(TypeId::of::<Config>()),
            mode: RequestMode::Inject,
            type_name: type_name::<Config>(),
        }]
    );
}

struct Seed;

fn make_seed() -> Result<Seed, InstantiateErrorKind> {
    Ok(Seed)
}

#[test]
fn keeps_inject_transient_as_a_transient_request() {
    let graph = registry! {
        scope(App) [
            provide(make_seed),
            provide(|InjectTransient(_seed): InjectTransient<Seed>| Ok::<_, InstantiateErrorKind>(Database)),
        ],
    }
    .graph();

    assert_eq!(graph.registrations[1].requests[0].mode, RequestMode::InjectTransient);
}

#[test]
fn marks_instance_as_a_runtime_value_source() {
    let graph = registry! {
        scope(App) [
            provide(make_config),
            provide(instance(Seed2)),
        ],
    }
    .graph();

    assert_eq!(graph.registrations[0].source, ValueSource::Factory);
    assert_eq!(graph.registrations[1].source, ValueSource::Instance);
}

#[derive(Clone)]
struct Seed2;

#[test]
fn records_where_each_registration_is_written() {
    let line = line!() + 3;
    let graph = registry! {
        scope(App) [
            provide(make_config),
        ],
    }
    .graph();

    let origin = graph.registrations[0].origin.expect("registry! records origins");
    assert_eq!(origin.expr, "make_config");
    assert_eq!(origin.file, file!());
    assert_eq!(origin.line, line);
}

#[test]
fn records_config_and_finalizer_in_any_order() {
    use froodi_compile::{thread_safety::RcThreadSafety, Config as Settings};

    let graph = registry! {
        scope(App) [
            provide(make_config, config = Settings { cache_provides: false }, finalizer = |_: RcThreadSafety<Config>| {}),
            provide(make_seed, finalizer = |_: RcThreadSafety<Seed>| {}, config = Settings { cache_provides: false }),
            provide(make_database),
        ],
    }
    .graph();

    let summary: Vec<_> = graph
        .registrations
        .iter()
        .map(|registration| (registration.cache_provides, registration.finalizer))
        .collect();
    assert_eq!(
        summary,
        vec![(false, Some(ExecutionKind::Sync)), (false, Some(ExecutionKind::Sync)), (true, None)]
    );
}

#[test]
fn numbers_extended_fragments_after_local_registrations_and_keeps_the_hierarchy() {
    let fragment = registry! {
        scope(App) [
            provide(make_config),
        ],
    };
    let graph = registry! {
        scope(App) [
            provide(make_database),
        ],
        extend(fragment),
    }
    .graph();

    let keys: Vec<_> = graph.registrations.iter().map(|registration| registration.key).collect();
    assert_eq!(keys, vec![TypeId::of::<Database>(), TypeId::of::<Config>()]);
    let names: Vec<_> = graph.scopes.iter().map(|scope| scope.name).collect();
    assert_eq!(names, vec!["runtime", "app", "session", "request", "action", "step"]);
}

#[test]
fn compiles_a_registry_graph_deterministically() {
    let make = || {
        registry! {
            scope(App) [
                provide(make_config),
                provide(make_database),
            ],
        }
        .graph()
    };

    let compiled = froodi_compile::ir::compile(make()).unwrap();
    assert_eq!(compiled, froodi_compile::ir::compile(make()).unwrap());
    assert_eq!(
        compiled.order(),
        &[froodi_compile::ir::RegistrationId(0), froodi_compile::ir::RegistrationId(1)]
    );
}

#[cfg(feature = "async")]
#[test]
fn marks_async_factories_and_finalizers_in_the_same_graph() {
    use froodi_compile::{async_registry, thread_safety::RcThreadSafety};

    let graph = async_registry! {
        scope(App) [
            provide(async || Ok::<_, InstantiateErrorKind>(Seed), finalizer = async |_: RcThreadSafety<Seed>| {}),
        ],
        extend(registry! { scope(App) [ provide(make_config) ] }),
    }
    .graph();

    let kinds: Vec<_> = graph
        .registrations
        .iter()
        .map(|registration| (registration.execution, registration.finalizer))
        .collect();
    assert_eq!(
        kinds,
        vec![(ExecutionKind::Async, Some(ExecutionKind::Async)), (ExecutionKind::Sync, None)]
    );
}

#[test]
fn labels_closure_and_call_origins_readably() {
    let graph = registry! {
        scope(App) [
            provide(|| Ok::<_, InstantiateErrorKind>(Seed)),
            provide(instance(Seed2)),
            provide(crate::make_config),
        ],
    }
    .graph();

    let labels: Vec<_> = graph
        .registrations
        .iter()
        .map(|registration| registration.origin.unwrap().expr)
        .collect();
    assert_eq!(labels, vec!["closure", "instance(..)", "crate::make_config"]);
}
