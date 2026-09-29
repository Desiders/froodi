use froodi_compile::{registry, Container, DefaultScope::App, Inject, InstantiateErrorKind};
struct A;
fn main() {
    let _ = Container::new(registry! { provide(App, |_: Inject<A>| Ok::<_, InstantiateErrorKind>(A)) });
}
#[test]
fn instantiates() {
    main();
}
