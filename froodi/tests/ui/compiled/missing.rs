use froodi::{Container, DefaultScope::App, Inject, InstantiateErrorKind};

fn main() {
    let _ = Container::new(froodi::compiled_registry! {
        provide(App, |_: Inject<u32>| Ok::<_, InstantiateErrorKind>(true)),
    });
}
