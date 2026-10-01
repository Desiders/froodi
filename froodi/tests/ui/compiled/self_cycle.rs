use froodi::compiled_registry as registry;
use froodi::{Container, DefaultScope::App, Inject, InstantiateErrorKind};

struct A;

fn main() {
    let _ = Container::new(registry! { provide(App, |_: Inject<A>| Ok::<_, InstantiateErrorKind>(A)) });
}
