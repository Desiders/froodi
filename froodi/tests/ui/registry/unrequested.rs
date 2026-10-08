use froodi::registry;
use froodi::{instance, Container, DefaultScope::App, InjectTransient, InstantiateErrorKind};

struct A;

fn main() {
    let container = Container::new(registry! {
        provide(App, instance(42u32)),
        provide(App, |_: InjectTransient<A>| Ok::<_, InstantiateErrorKind>(A)),
    });
    assert_eq!(*container.get::<u32>().unwrap(), 42);
}
