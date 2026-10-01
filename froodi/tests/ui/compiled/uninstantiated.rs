#![allow(dead_code)]
use froodi::compiled_registry as registry;
use froodi::{Container, DefaultScope::App, Inject, InstantiateErrorKind, Instantiator};

struct A;

fn wrap<Inst: Instantiator<(Inject<A>,), Provides = A, Error = InstantiateErrorKind> + Send + Sync>(inst: Inst) -> Container {
    Container::new(registry! { provide(App, inst) })
}

fn unused() {
    let _ = Container::new(registry! { provide(App, |_: Inject<A>| Ok::<_, InstantiateErrorKind>(A)) });
}

fn main() {}
