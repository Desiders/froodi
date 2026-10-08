use di::{Container, DefaultScope::App, Inject, InstantiateErrorKind};

type Number = u32;

fn main() {
    let _ = Container::new(di::registry! {
        provide(App, || Ok::<_, InstantiateErrorKind>(1u32)),
        provide(App, || Ok::<Number, InstantiateErrorKind>(2)),
        provide(App, |_: Inject<u32>| Ok::<_, InstantiateErrorKind>(true)),
    });
}
