//! `extend(...)`: an inline registry is analysed; any other argument builds registrations at runtime.

use froodi_compile_build::{analyze, ExtensionOutcome, Outcome, Reason, RegistryKind};

const SOURCE: &str = r"
fn make_cache() -> InstantiatorResult<Cache> { todo!() }

async fn build(config: Config) -> AsyncRegistry {
    async_registry! {
        provide(App, instance(config)),
        extend(
            infrastructure(),
            registry! { provide(App, make_cache) },
            if config.metrics { metrics() } else { Registry::default() },
        ),
    }
}
";

#[test]
fn a_runtime_argument_is_unresolved_and_an_inline_registry_is_analysed() {
    let analysis = analyze(SOURCE).unwrap();
    let extensions = &analysis.registries[0].extensions;
    let outcomes: Vec<_> = extensions
        .iter()
        .map(|extension| match &extension.outcome {
            ExtensionOutcome::Inline(nested) => Some((nested.kind, nested.registrations.len())),
            ExtensionOutcome::Unresolved(reason) => {
                assert_eq!(reason, &Reason::ExtendIsARuntimeExpression);
                None
            }
        })
        .collect();
    assert_eq!(outcomes, [None, Some((RegistryKind::Sync, 1)), None]);
    assert_eq!(extensions[1].line, 9);

    let nested = analysis.registrations()[1];
    assert_eq!(
        nested.outcome,
        Outcome::Resolved {
            provides: "Cache".to_owned(),
            deps: vec![]
        }
    );
}

#[test]
fn the_summary_counts_registrations_and_runtime_extensions() {
    let summary = analyze(SOURCE).unwrap().summary();
    assert_eq!(summary.resolved, 1);
    assert_eq!(summary.unresolved["InstanceTypeNotWritten"], 1);
    assert_eq!(summary.unresolved["ExtendIsARuntimeExpression"], 2);
    assert_eq!(summary.total(), 4);
}
