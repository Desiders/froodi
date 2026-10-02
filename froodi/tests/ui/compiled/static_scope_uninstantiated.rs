use froodi as scope_api;
use froodi::{compiled_registry as registry, instance, Container, Inject, InstantiateErrorKind};

#[path = "support/scopes.rs"]
mod scopes;
use scopes::{App, Request};

fn unused() -> Container {
    Container::new(registry! {
        provide(App, |_: Inject<u32>| Ok::<_, InstantiateErrorKind>(String::new())),
        provide(Request, instance(7u32)),
    })
}

fn main() {}
