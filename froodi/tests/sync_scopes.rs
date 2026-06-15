#![no_std]

extern crate alloc;

use alloc::string::ToString as _;
use froodi::{
    registry, Container, Context,
    DefaultScope::{self, Action, App, Request, Runtime, Session, Step},
    ResolveErrorKind, Scope, ScopeErrorKind, ScopeWithErrorKind, Scopes,
};

struct AppDep(u32);
struct RequestDep(u32);
struct ActionDep(u32);
struct StepDep(u32);

fn build_registry() -> froodi::Registry {
    registry! {
        scope(App) [
            provide(|| Ok(AppDep(1))),
        ],
        scope(Request) [
            provide(|| Ok(RequestDep(2))),
        ],
        scope(Action) [
            provide(|| Ok(ActionDep(3))),
        ],
        scope(Step) [
            provide(|| Ok(StepDep(4))),
        ],
    }
}

#[test]
fn default_scope_trait_values() {
    assert_eq!(Runtime.name(), "runtime");
    assert_eq!(App.name(), "app");
    assert_eq!(Session.name(), "session");
    assert_eq!(Request.name(), "request");
    assert_eq!(Action.name(), "action");
    assert_eq!(Step.name(), "step");

    assert!(Runtime.priority() < App.priority());
    assert!(App.priority() < Session.priority());
    assert!(Session.priority() < Request.priority());
    assert!(Request.priority() < Action.priority());
    assert!(Action.priority() < Step.priority());

    assert_eq!(Runtime.priority(), 0);
    assert_eq!(App.priority(), 1);
    assert_eq!(Session.priority(), 2);
    assert_eq!(Request.priority(), 3);
    assert_eq!(Action.priority(), 4);
    assert_eq!(Step.priority(), 5);

    assert!(Runtime.is_skipped_by_default());
    assert!(Session.is_skipped_by_default());
    assert!(!App.is_skipped_by_default());
    assert!(!Request.is_skipped_by_default());
    assert!(!Action.is_skipped_by_default());
    assert!(!Step.is_skipped_by_default());

    assert!(Runtime < Step);
    assert!(App < Request);
    assert!(Session < Action);
}

#[test]
fn default_scope_scopes_all() {
    let (first, rest) = <DefaultScope as Scopes<5>>::all();
    // DefaultScope does not implement Debug, so compare with `==`.
    assert!(first == Runtime);
    assert!(rest == [App, Session, Request, Action, Step]);
    for scope in rest {
        assert!(first.priority() < scope.priority());
    }
}

#[test]
fn new_lands_at_app_and_descends() {
    let app_container = Container::new(build_registry());

    let app_dep = app_container.get::<AppDep>().expect("App dep resolves at App container");
    assert_eq!(app_dep.0, 1);

    let request_container = app_container.clone().enter_build().expect("App descends to Request");
    let request_dep = request_container.get::<RequestDep>().expect("Request dep resolves");
    assert_eq!(request_dep.0, 2);

    let action_container = request_container.clone().enter_build().expect("Request descends to Action");
    let action_dep = action_container.get::<ActionDep>().expect("Action dep resolves");
    assert_eq!(action_dep.0, 3);

    let step_container = action_container.clone().enter_build().expect("Action descends to Step");
    let step_dep = step_container.get::<StepDep>().expect("Step dep resolves");
    assert_eq!(step_dep.0, 4);
}

#[test]
fn new_with_start_scope_request() {
    let request_container = Container::new_with_start_scope(build_registry(), Request);

    let request_dep = request_container.get::<RequestDep>().expect("Request dep resolves directly");
    assert_eq!(request_dep.0, 2);

    let app_dep = request_container.get::<AppDep>().expect("App dep accessible from parent");
    assert_eq!(app_dep.0, 1);
}

#[test]
fn enter_with_scope_request_from_app() {
    let app_container = Container::new(build_registry());

    let request_container = app_container
        .clone()
        .enter()
        .with_scope(Request)
        .build()
        .expect("with_scope(Request) succeeds from App");
    let request_dep = request_container.get::<RequestDep>().expect("Request dep resolves");
    assert_eq!(request_dep.0, 2);

    let request_container_ctx = app_container
        .clone()
        .enter()
        .with_context(Context::new())
        .with_scope(Request)
        .build()
        .expect("with_context + with_scope(Request) succeeds");
    let request_dep_ctx = request_container_ctx.get::<RequestDep>().expect("Request dep resolves with ctx");
    assert_eq!(request_dep_ctx.0, 2);

    let request_container_ctx2 = app_container
        .enter()
        .with_scope(Request)
        .with_context(Context::new())
        .build()
        .expect("with_scope(Request) + with_context succeeds");
    let request_dep_ctx2 = request_container_ctx2.get::<RequestDep>().expect("Request dep resolves with ctx2");
    assert_eq!(request_dep_ctx2.0, 2);
}

#[test]
fn error_no_child_registries_at_leaf() {
    let app_container = Container::new(build_registry());
    let step_container = app_container
        .enter_build()
        .expect("App -> Request")
        .enter_build()
        .expect("Request -> Action")
        .enter_build()
        .expect("Action -> Step");

    // Container does not implement Debug, so use `.err()` instead of expect_err.
    let err = step_container.enter_build().err().expect("descending past leaf Step must fail");
    assert!(matches!(err, ScopeErrorKind::NoChildRegistries));
    assert!(!err.to_string().is_empty());
}

#[test]
fn error_no_child_registries_with_scope_for_own_scope() {
    let app_container = Container::new(build_registry());

    // with_scope on the own scope must fail: descent starts at the first child, never reaching App's priority.
    let err = app_container
        .enter()
        .with_scope(App)
        .build()
        .err()
        .expect("with_scope(App) from an App container must fail");
    assert!(matches!(err, ScopeWithErrorKind::NoChildRegistriesWithScope { .. }));

    // Compute the Display string before destructuring moves `err`.
    let display = err.to_string();
    assert!(display.contains("app"));

    if let ScopeWithErrorKind::NoChildRegistriesWithScope { name, priority } = err {
        assert_eq!(name, "app");
        assert_eq!(priority, App.priority());
    } else {
        unreachable!("error variant already asserted above");
    }
}

#[test]
fn unregistered_type_no_instantiator() {
    #[derive(Debug)]
    struct Unregistered;

    let app_container = Container::new(build_registry());
    let err = app_container
        .get::<Unregistered>()
        .expect_err("unregistered type has no instantiator");
    assert!(matches!(err, ResolveErrorKind::NoInstantiator { .. }));
}
