use froodi as scope_api;
use froodi::{registry as registry, instance, Container, InjectTransient, InstantiateErrorKind};

#[path = "support/scopes.rs"]
mod scopes;
use scopes::{App, Request};

fn main() {
    let fragment = registry! { provide(App, |_: InjectTransient<u32>| Ok::<_, InstantiateErrorKind>(String::new())) };
    let _ = Container::new(registry! { extend(fragment), provide(Request, instance(7u32)) });
}
