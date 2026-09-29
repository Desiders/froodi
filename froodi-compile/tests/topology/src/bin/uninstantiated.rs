#![allow(dead_code)]
use froodi_compile::{registry, Container, DefaultScope::App, Inject, InstantiateErrorKind, Instantiator};
struct A;
fn wrap<Inst: Instantiator<(Inject<A>,), Provides = A> + Send + Sync>(inst: Inst) -> Container {
    Container::new(registry! { provide(App, inst) })
}
fn unused() {
    let _ = Container::new(registry! { provide(App, |_: Inject<A>| Ok::<_, InstantiateErrorKind>(A)) });
}
fn main() {}
#[test]
fn does_not_instantiate() {
    main();
}
