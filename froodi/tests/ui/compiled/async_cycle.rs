use froodi::compiled_async_registry as async_registry;
use froodi::{async_impl::Container, DefaultScope::App, InjectTransient, InstantiateErrorKind};

struct A;

fn main() {
    let _ = Container::new(async_registry! { provide(App, async |_: InjectTransient<A>| Ok::<_, InstantiateErrorKind>(A)) });
}
