use froodi::{registry, DefaultScope::App, Inject, InstantiateErrorKind, TypedContainer};

struct Service;

fn service(_: Inject<Service>) -> Result<Service, InstantiateErrorKind> {
    Ok(Service)
}

fn main() {
    let _ = TypedContainer::new(registry! { provide(App, service) });
}
