//! Finding `registry!` invocations and reading their clauses.

use froodi_compile_build::{analyze, ExtensionOutcome, RegistryKind};

const SOURCE: &str = r"
use froodi::{registry, async_registry, instance, Inject, InstantiatorResult, DefaultScope::{App, Request}};

fn make_database(Inject(config): Inject<Config>) -> InstantiatorResult<Database> {
    Ok(Database::connect(&config.url))
}

fn build(config: Config) -> Registry {
    registry! {
        provide(App, instance(config)),
        scope(Request) [
            provide(make_database),
            provide(make_handler, config = Config { cache_provides: false }, finalizer = fin),
        ],
        extend(infrastructure()),
    }
}

async fn build_async() -> AsyncRegistry {
    froodi::async_registry! {
        provide(DefaultScope::App, make_database, finalizer = close),
    }
}
";

#[test]
fn finds_every_invocation_with_its_kind_and_line() {
    let analysis = analyze(SOURCE).unwrap();
    let found: Vec<_> = analysis.registries.iter().map(|registry| (registry.kind, registry.line)).collect();
    assert_eq!(found, [(RegistryKind::Sync, 9), (RegistryKind::Async, 20)]);
}

#[test]
fn reads_scope_blocks_and_top_level_provides_in_source_order() {
    let analysis = analyze(SOURCE).unwrap();
    let registry = &analysis.registries[0];
    let entries: Vec<_> = registry
        .registrations
        .iter()
        .map(|registration| (registration.scope.as_str(), registration.factory.as_str(), registration.line))
        .collect();
    assert_eq!(
        entries,
        [
            ("App", "instance(config)", 10),
            ("Request", "make_database", 12),
            ("Request", "make_handler", 13),
        ]
    );
}

#[test]
fn reads_config_and_finalizer_options() {
    let analysis = analyze(SOURCE).unwrap();
    let options: Vec<_> = analysis
        .registrations()
        .iter()
        .map(|registration| (registration.has_config, registration.has_finalizer))
        .collect();
    assert_eq!(options, [(false, false), (false, false), (true, true), (false, true)]);
    assert_eq!(analysis.registries[1].registrations[0].scope, "DefaultScope::App");
}

#[test]
fn reads_extend_arguments() {
    let analysis = analyze(SOURCE).unwrap();
    let extensions = &analysis.registries[0].extensions;
    assert_eq!(extensions.len(), 1);
    assert_eq!(extensions[0].expr, "infrastructure()");
    assert_eq!(extensions[0].line, 15);
    assert!(matches!(extensions[0].outcome, ExtensionOutcome::Unresolved(_)));
}

#[test]
fn ignores_other_macros_and_rejects_malformed_registry_bodies() {
    let analysis = analyze("fn main() { println!(\"registry\"); vec![1, 2]; }").unwrap();
    assert!(analysis.registries.is_empty());

    let error = analyze("fn main() { registry! { provide(App) } }").unwrap_err();
    assert_eq!(error.span().start().line, 1);
}
