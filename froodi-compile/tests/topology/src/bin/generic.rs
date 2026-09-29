use froodi_compile::{registry, Container, DefaultScope::App, Inject, InstantiateErrorKind, Instantiator};

struct A;

fn wrap<Inst: Instantiator<(Inject<A>,), Provides = A> + Send + Sync>(inst: Inst) -> Container {
    Container::new(registry! { provide(App, inst) })
}

fn main() {
    let _ = wrap(|_: Inject<A>| Ok::<_, InstantiateErrorKind>(A));
}

#[test]
fn instantiates() {
    main();
}
