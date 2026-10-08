use froodi::{declare, instance, registry, DefaultScope::App, TypedContainer};

fn main() {
    let _ = TypedContainer::new(registry! { provide(App, declare::<u32>()) });
    let _ = TypedContainer::new(registry! {
        extend(registry! { provide(App, instance(7u32)) }),
    });
}
