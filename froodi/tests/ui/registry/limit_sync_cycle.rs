use froodi::{registry, Container, DefaultScope::App, Inject, InstantiateErrorKind};

include!("../../fixtures/padding.rs");

struct Cycle;

fn main() {
    // Keep the complete graph at 1,024 nodes, including implicit containers.
    #[cfg(not(feature = "async"))]
    let padding = padding_1022!();
    #[cfg(feature = "async")]
    let padding = padding_1021!();

    let _ = Container::new(registry! {
        provide(App, |_: Inject<Cycle>| Ok::<_, InstantiateErrorKind>(Cycle)),

        extend(padding),
    });
}
