#![no_std]

extern crate alloc;

use froodi::{
    registry, Container,
    DefaultScope::{App, Request},
    Inject, ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind,
};

use alloc::{
    format,
    string::{String, ToString as _},
};

#[derive(Debug)]
struct Missing;

#[derive(Debug)]
struct RequestOnly;

#[derive(Debug)]
struct DependsOnRequestOnly(#[allow(dead_code)] RcThreadSafety<RequestOnly>);

use froodi::utils::thread_safety::RcThreadSafety;

fn app_container() -> Container {
    Container::new(registry! {
        scope(App) [
            provide(|| Ok(Marker)),
        ],
        scope(Request) [
            provide(|| Ok(RequestOnly)),
            provide(|Inject(r): Inject<RequestOnly>| Ok(DependsOnRequestOnly(r))),
        ],
    })
}

struct Marker;

#[test]
fn no_instantiator_variant_and_display() {
    let container = app_container();

    let err = container.get::<Missing>().unwrap_err();

    assert!(
        matches!(err, ResolveErrorKind::NoInstantiator { .. }),
        "expected NoInstantiator, got: {err:?}",
    );

    let display = err.to_string();
    assert!(display.contains("not found"), "display missing 'not found': {display}");
    assert!(display.contains("Missing"), "display missing type name 'Missing': {display}");

    let debug = format!("{err:?}");
    assert!(!debug.is_empty(), "debug must be non-empty");
    assert!(debug.contains("NoInstantiator"), "debug should name the variant: {debug}");

    let err_t = container.get_transient::<Missing>().unwrap_err();
    assert!(
        matches!(err_t, ResolveErrorKind::NoInstantiator { .. }),
        "expected NoInstantiator from get_transient, got: {err_t:?}",
    );
}

#[test]
fn no_accessible_variant_and_display_for_get() {
    let app = app_container();

    // RequestOnly lives at a child (Request) scope, so resolving from App fails NoAccessible.
    let err = app.get::<RequestOnly>().unwrap_err();

    assert!(
        matches!(
            err,
            ResolveErrorKind::NoAccessible {
                expected_scope_data: _,
                actual_scope_data: _,
            }
        ),
        "expected NoAccessible, got: {err:?}",
    );

    let display = err.to_string();
    assert!(
        display.to_lowercase().contains("accessible"),
        "display should mention 'accessible': {display}",
    );
    assert!(display.contains("app"), "display should mention actual scope 'app': {display}");
    assert!(
        display.contains("request"),
        "display should mention expected scope 'request': {display}",
    );

    let debug = format!("{err:?}");
    assert!(!debug.is_empty(), "debug must be non-empty");
    assert!(debug.contains("NoAccessible"), "debug should name the variant: {debug}");

    let err_dep = app.get::<DependsOnRequestOnly>().unwrap_err();
    assert!(
        matches!(err_dep, ResolveErrorKind::NoAccessible { .. }),
        "expected NoAccessible for dependent, got: {err_dep:?}",
    );
}

#[test]
fn no_accessible_variant_for_get_transient() {
    let app = app_container();

    let err = app.get_transient::<RequestOnly>().unwrap_err();

    assert!(
        matches!(
            err,
            ResolveErrorKind::NoAccessible {
                expected_scope_data: _,
                actual_scope_data: _,
            }
        ),
        "expected NoAccessible from get_transient, got: {err:?}",
    );

    let display = err.to_string();
    assert!(
        display.to_lowercase().contains("accessible"),
        "display should mention 'accessible': {display}",
    );

    let debug = format!("{err:?}");
    assert!(!debug.is_empty(), "debug must be non-empty for get_transient NoAccessible");
}

#[test]
fn no_accessible_succeeds_from_request_child() {
    let app = app_container();
    let request = app.enter().with_scope(Request).build().unwrap();

    assert!(request.get::<RequestOnly>().is_ok());
    assert!(request.get::<DependsOnRequestOnly>().is_ok());
    assert!(request.get_transient::<RequestOnly>().is_ok());
}

#[test]
fn scope_error_no_child_registries_at_leaf() {
    let app = app_container();
    let request = app.enter_build().unwrap();
    let action = request.enter_build().unwrap();
    let step = action.enter_build().unwrap(); // Step is the leaf scope

    // Container does not implement Debug, so unwrap the error via `.err()`.
    let err = step.enter_build().err().unwrap();

    assert!(
        matches!(err, ScopeErrorKind::NoChildRegistries),
        "expected ScopeErrorKind::NoChildRegistries, got: {err:?}",
    );

    let display = err.to_string();
    assert!(!display.is_empty(), "display must be non-empty");
    assert!(
        display.contains("Child registries"),
        "display should contain 'Child registries': {display}",
    );

    let debug = format!("{err:?}");
    assert!(debug.contains("NoChildRegistries"), "debug should name the variant: {debug}");
}

#[test]
fn scope_with_error_no_child_registries_with_scope_for_own_scope() {
    // with_scope requires a strict descendant, so requesting the container's own scope (App) fails.
    let app = app_container();

    let err = app.enter().with_scope(App).build().err().unwrap();

    assert!(
        matches!(err, ScopeWithErrorKind::NoChildRegistriesWithScope { .. }),
        "expected NoChildRegistriesWithScope, got: {err:?}",
    );

    let display = err.to_string();
    // Display format: "Registry with name app and priority N not found in container"
    assert!(display.contains("app"), "display should contain scope name 'app': {display}");
    assert!(display.contains("not found"), "display should contain 'not found': {display}");

    if let ScopeWithErrorKind::NoChildRegistriesWithScope { name, priority } = err {
        assert_eq!(name, "app", "name field should be the App scope name");
        // App priority is 1 in DefaultScope ordering.
        assert_eq!(priority, 1, "App priority should be 1");
    } else {
        panic!("variant changed unexpectedly");
    }

    let debug = format!("{:?}", ScopeWithErrorKind::NoChildRegistriesWithScope { name: "app", priority: 1 });
    assert!(!debug.is_empty(), "debug must be non-empty");
    assert!(
        debug.contains("NoChildRegistriesWithScope"),
        "debug should name the variant: {debug}",
    );
}

#[test]
fn scope_with_error_descendant_scope_succeeds() {
    let app = app_container();
    assert!(app.enter().with_scope(Request).build().is_ok());
}

#[test]
fn display_strings_are_distinct_across_error_kinds() {
    let app = app_container();

    let no_inst: String = app.get::<Missing>().unwrap_err().to_string();
    let no_acc: String = app.get::<RequestOnly>().unwrap_err().to_string();
    let scope_with: String = app.clone().enter().with_scope(App).build().err().unwrap().to_string();

    assert_ne!(no_inst, no_acc);
    assert_ne!(no_inst, scope_with);
    assert_ne!(no_acc, scope_with);

    assert!(!no_inst.is_empty());
    assert!(!no_acc.is_empty());
    assert!(!scope_with.is_empty());
}
