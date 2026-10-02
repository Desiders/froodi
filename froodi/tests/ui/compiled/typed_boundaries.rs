use froodi::{compiled_registry, context, instance, registry, runtime, DefaultScope::App, TypedContainer};

fn main() {
    let _ = TypedContainer::new(compiled_registry! { provide(App, runtime::<u32>()) });
    let _ = TypedContainer::new(compiled_registry! { provide(App, context::<u32>()) });
    let _ = TypedContainer::new(compiled_registry! {
        extend(registry! { provide(App, instance(7u32)) }),
    });
}
