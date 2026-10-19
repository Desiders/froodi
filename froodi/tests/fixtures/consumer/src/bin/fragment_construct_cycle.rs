use di::utils::thread_safety::RcThreadSafety;
use di::{fragment, registry, Container, DefaultScope::App};

#[derive(di::Construct)]
struct Foo {
    bar: RcThreadSafety<Bar>,
}
#[derive(di::Construct)]
struct Bar {
    foo: RcThreadSafety<Foo>,
}
#[fragment(first)]
registry! { scope(App) [ provide::<Foo>(), ], }
#[fragment(second)]
registry! { scope(App) [ provide::<Bar>(), ], }

fn main() {
    let _ = Container::new(registry! { extend_fragment(first!(), second!()), });
}
