use di::{fragment, registry, Container, DefaultScope::App, InjectTransient, InstantiateErrorKind};

#[fragment(app)]
registry! { provide(App, |_: InjectTransient<u32>| Ok::<_, InstantiateErrorKind>(true)), }

fn main() {
    let _ = Container::new(registry! { extend_fragment(app!()), });
}
