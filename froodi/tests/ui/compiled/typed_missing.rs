use froodi::{compiled_registry, instance, DefaultScope::App, TypedContainer, TypedContainerExt as _};

struct Missing;

fn main() {
    let container = TypedContainer::new(compiled_registry! { provide(App, instance(7u32)) });
    let _ = container.get::<Missing>();
    let _ = container.get_transient::<Missing>();
    let _ = container.clone().enter_build().unwrap().get::<Missing>();
}
