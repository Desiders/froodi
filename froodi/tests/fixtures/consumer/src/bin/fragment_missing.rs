use di::{fragment, registry, Container, DefaultScope::App, Inject, InstantiateErrorKind};

#[fragment(app)]
registry! { provide(App, |_: Inject<u32>| Ok::<_, InstantiateErrorKind>(true)), }

fn main() {
    let _ = Container::new(registry! { extend_fragment(app!()), });
}
