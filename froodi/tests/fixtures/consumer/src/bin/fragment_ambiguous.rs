use di::{fragment, instance, registry, Container, DefaultScope::App, Inject, InstantiateErrorKind};

#[fragment(first)]
registry! { provide(App, instance(1u32)), }
#[fragment(second)]
registry! { provide(App, instance(2u32)), }

fn main() {
    let _ = Container::new(registry! {
        extend_fragment(first!(), second!()),
        provide(App, |_: Inject<u32>| Ok::<_, InstantiateErrorKind>(true)),
    });
}
