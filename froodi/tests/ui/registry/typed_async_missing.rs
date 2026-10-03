use froodi::{
    async_impl::{TypedContainer, TypedContainerExt as _},
    async_registry,
    DefaultScope::App,
};

struct Missing;

fn main() {
    let container = TypedContainer::new(async_registry! { provide(App, async || Ok(7u32)) });
    let _ = container.get::<Missing>();
    let _ = container.get_transient::<Missing>();
    let _ = container.clone().enter_build().unwrap().get::<Missing>();
}
