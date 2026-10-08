use di::{fragment, registry, Container, DefaultScope::App, Inject, InstantiateErrorKind};

struct Foo;
struct Bar;
#[fragment(first)]
registry! { provide(App, |_: Inject<Bar>| Ok::<_, InstantiateErrorKind>(Foo)), }
#[fragment(second)]
registry! { provide(App, |_: Inject<Foo>| Ok::<_, InstantiateErrorKind>(Bar)), }

fn main() {
    let _ = Container::new(registry! { extend_fragment(first!(), second!()), });
}
