use froodi_compile::{
    async_impl::{ConstructAsyncRegistration, Container},
    registry, runtime,
    DefaultScope::App,
};

fn main() {
    let (leaf, _) = registry! { provide(App, runtime::<String>()) }.into_parts();
    let container = Container::new(registry!());
    // Erased construction requires a proven correspondence; safe callers cannot supply arbitrary edges.
    let _ = leaf.construct_async(&container, &[]);
}
