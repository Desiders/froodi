use froodi as scope_api;
use froodi::{Container, instance};

#[path = "support/scopes.rs"]
mod scopes;
use scopes::{App, Request};

#[derive(froodi::Construct)]
struct Service {
    #[di(inject_transient)]
    request_id: u32,
}

fn main() {
    let _ = Container::new(froodi::registry! {
        scope(App) [ construct::<Service>() ],
        provide(Request, instance(7u32)),
    });
}
