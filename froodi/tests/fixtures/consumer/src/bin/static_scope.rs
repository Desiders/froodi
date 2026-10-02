use compiled_enabler::scopes::{App, Request};
use di::{compiled_registry as renamed_registry, instance, Container, Inject, InstantiateErrorKind};

fn service(_: Inject<u32>) -> Result<String, InstantiateErrorKind> {
    Ok(String::new())
}

fn main() {
    let _ = Container::new(renamed_registry! { provide(App, service), provide(Request, instance(7u32)) });
}

#[test]
fn instantiated_constructor() {
    main();
}
