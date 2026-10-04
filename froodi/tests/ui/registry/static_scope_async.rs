use froodi as scope_api;
use froodi::{async_impl::Container, registry, Inject, InstantiateErrorKind};

#[path = "support/scopes.rs"]
mod scopes;
use scopes::{App, Request};

fn main() {
    let _ = Container::new(registry! {
        provide(App, async |_: Inject<u32>| Ok::<_, InstantiateErrorKind>(String::new())),
        provide(Request, async || Ok::<_, InstantiateErrorKind>(7u32)),
    });
}
