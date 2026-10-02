use froodi::{compiled_registry, instance, DefaultScope::App, TypedContainer};

fn same_providers<Providers>(_: &TypedContainer<Providers>, container: TypedContainer<Providers>) -> TypedContainer<Providers> {
    container
}

fn accept(_: &str) {}

fn main() {
    let general = TypedContainer::new(compiled_registry! { provide(App, instance(accept as fn(&str))) });
    let specific = TypedContainer::new(compiled_registry! { provide(App, instance(accept as fn(&'static str))) });
    let _ = same_providers(&specific, general);
}
