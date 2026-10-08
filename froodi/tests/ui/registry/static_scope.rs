use froodi as scope_api;
use froodi::{registry, instance, Container, Inject, InstantiateErrorKind};

#[path = "support/scopes.rs"]
mod scopes;
use scopes::{App, Request};

fn service(_: Inject<u32>) -> Result<String, InstantiateErrorKind> {
    Ok(String::new())
}

fn main() {
    let _ = Container::new(registry! { provide(App, service), provide(Request, instance(7u32)) });
}
