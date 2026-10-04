use froodi::{registry, Container, DefaultScope::App, InstantiateErrorKind};

fn main() {
    let _ = Container::new(registry! {
        provide(App, || Ok::<_, InstantiateErrorKind>(7u32), finalizer = |_| async {}),
    });
}
