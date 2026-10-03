use froodi::{registry as registry, Container, DefaultScope::App, Inject, InstantiateErrorKind};

include!("../../fixtures/padding.rs");

struct Cycle;

fn main() {
    // 1,022 padding leaves + Cycle + one implicit container = 1,024.

    let _ = Container::new(registry! {
        provide(App, |_: Inject<Cycle>| Ok::<_, InstantiateErrorKind>(Cycle)),

        extend(padding_1022!()),
    });
}
