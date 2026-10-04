use froodi::{registry, Container, DefaultScope::App, InstantiateErrorKind};

async fn unrelated() -> Result<String, InstantiateErrorKind> {
    Ok(String::new())
}

fn main() {
    let _ = Container::new(registry! {
        provide(App, || Ok::<_, InstantiateErrorKind>(7u32)),
        provide(App, unrelated),
    });
}
