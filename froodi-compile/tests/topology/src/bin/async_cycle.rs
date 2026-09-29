use froodi_compile::{async_impl::Container, async_registry, DefaultScope::App, InjectTransient, InstantiateErrorKind};

struct A;

fn main() {
    let _ = Container::new(async_registry! { provide(App, async |_: InjectTransient<A>| Ok::<_, InstantiateErrorKind>(A)) });
}

#[test]
fn instantiates() {
    main();
}
