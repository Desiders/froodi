use froodi::registry;
use froodi::{async_impl::Container, DefaultScope::App, InjectTransient, InstantiateErrorKind};

struct A;

fn main() {
    let _ = Container::new(registry! { provide(App, async |_: InjectTransient<A>| Ok::<_, InstantiateErrorKind>(A)) });
}
