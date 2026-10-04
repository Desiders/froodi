use froodi::{registry, DefaultScope::App, InstantiateErrorKind, TypedContainer, TypedContainerExt as _};

async fn unrelated() -> Result<String, InstantiateErrorKind> {
    Ok(String::new())
}

fn main() {
    let container = TypedContainer::new(registry! {
        provide(App, || Ok::<_, InstantiateErrorKind>(7u32)),
        provide(App, unrelated),
    });
    let _ = container.get::<String>();
}
