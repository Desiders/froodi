use froodi as scope_api;
use froodi::{compiled_registry as registry, instance, Container, Inject, InstantiateErrorKind, ScopeData, Scopes, StaticScope};

#[path = "support/scopes.rs"]
mod scopes;
use scopes::{App, Request};

fn build<S: StaticScope + Scopes<3, Scope = ScopeData>>(scope: S) -> Container {
    Container::new(registry! {
        provide(scope, |_: Inject<u32>| Ok::<_, InstantiateErrorKind>(String::new())),
        provide(Request, instance(7u32)),
    })
}

fn main() {
    let _ = build(App);
}
