use di::{fragment, registry, DefaultScope::App, Inject, InstantiateErrorKind};

async fn database() -> Result<u32, InstantiateErrorKind> {
    Ok(7)
}
#[fragment(infrastructure)]
registry! { provide(App, database), }
#[fragment(app)]
registry! { provide(App, |_: Inject<u32>| Ok::<_, InstantiateErrorKind>(true)), }

fn main() {
    let _ = di::async_impl::Container::new(registry! { extend_fragment(infrastructure!(), app!()), });
}
