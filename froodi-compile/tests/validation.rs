//! Graph checks that run when a container is built: what rustc cannot see.

#![allow(clippy::unnecessary_wraps, reason = "factories return Result by contract")]

use froodi_compile::{ir::Diagnostic, registry, Container, DefaultScope::*, Inject, InstantiateErrorKind};

struct Config;
struct Handler;

#[test]
fn reports_a_duplicate_nothing_depends_on() {
    let result = Container::try_new(registry! {
        scope(App) [
            provide(|| Ok::<_, InstantiateErrorKind>(Config)),
            provide(|| Ok::<_, InstantiateErrorKind>(Config)),
        ],
    });

    let diagnostics = result.err().expect("the duplicate is reported");
    assert!(matches!(diagnostics.0.as_slice(), [Diagnostic::Duplicate { origins, .. }] if origins.len() == 2));
}

#[test]
fn reports_a_dependency_on_a_narrower_scope() {
    let result = Container::try_new(registry! {
        provide(Request, || Ok::<_, InstantiateErrorKind>(Handler)),
        provide(App, |Inject(_handler): Inject<Handler>| Ok::<_, InstantiateErrorKind>(Config)),
    });

    let diagnostics = result.err().expect("the scope violation is reported");
    assert!(matches!(diagnostics.0.as_slice(), [Diagnostic::ScopeViolation { .. }]));
}

#[test]
#[should_panic(expected = "Config is provided by 2 registrations")]
fn new_panics_with_the_rendered_diagnostics() {
    let _container = Container::new(registry! {
        scope(App) [
            provide(|| Ok::<_, InstantiateErrorKind>(Config)),
            provide(|| Ok::<_, InstantiateErrorKind>(Config)),
        ],
    });
}
