use froodi::{
    registry as registry, instance, Container, DefaultScope::App, DependencyResolver, Inject, InjectTransient,
    InstantiateErrorKind, ResolveErrorKind, InjectCustom,
};

struct Opaque;
struct A;
struct B;

impl DependencyResolver for Opaque {
    type Error = ResolveErrorKind;

    fn resolve(_: &Container) -> Result<Self, Self::Error> {
        Ok(Self)
    }

    #[cfg(feature = "async")]
    async fn resolve_async(_: &froodi::async_impl::Container) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

fn main() {
    let fragment = registry! {
        provide(App, |_: InjectCustom<Opaque>, _: Inject<B>| Ok::<_, InstantiateErrorKind>(A)),
    };
    let container = Container::new(registry! {
        provide(App, instance(7u32)),
        extend(fragment),
        provide(App, |_: InjectTransient<A>| Ok::<_, InstantiateErrorKind>(B)),
    });
    assert_eq!(*container.get::<u32>().unwrap(), 7);
}
