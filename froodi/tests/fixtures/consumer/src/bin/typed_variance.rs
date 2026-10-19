use di::{instance, registry, DefaultScope::App, TypedContainer};

fn same_providers<Providers>(_: &TypedContainer<Providers>, container: TypedContainer<Providers>) -> TypedContainer<Providers> {
    container
}

fn accept(_: &str) {}

fn main() {
    let general = TypedContainer::new(registry! { provide(App, instance(accept as fn(&str))) });
    let specific = TypedContainer::new(registry! { provide(App, instance(accept as fn(&'static str))) });
    let _ = same_providers(&specific, general);
}
