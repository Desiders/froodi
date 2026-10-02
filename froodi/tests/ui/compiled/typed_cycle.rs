use froodi::{compiled_registry, DefaultScope::App, Inject, InstantiateErrorKind, TypedContainer};

struct Service;

fn service(_: Inject<Service>) -> Result<Service, InstantiateErrorKind> {
    Ok(Service)
}

fn main() {
    let _ = TypedContainer::new(compiled_registry! { provide(App, service) });
}
