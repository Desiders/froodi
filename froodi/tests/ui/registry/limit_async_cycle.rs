use froodi::{async_impl::Container, registry, DefaultScope::App, Inject, InstantiateErrorKind};

include!("../../fixtures/padding.rs");

struct Cycle;

fn main() {
    // 1,021 padding leaves + Cycle + two implicit containers = 1,024.

    let _ = Container::new(registry! {
        provide(App, async |_: Inject<Cycle>| Ok::<_, InstantiateErrorKind>(Cycle)),

        extend(padding_1021!()),
    });
}
