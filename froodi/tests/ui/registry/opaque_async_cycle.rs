use froodi::{
    async_impl::Container, async_registry as async_registry, DefaultScope::App, DependencyResolver, InjectTransient,
    InstantiateErrorKind, ResolveErrorKind, InjectCustom,
};

struct Opaque;
struct A;

impl DependencyResolver for Opaque {
    type Error = ResolveErrorKind;

    fn resolve(_: &froodi::Container) -> Result<Self, Self::Error> {
        Ok(Self)
    }

    async fn resolve_async(_: &Container) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

fn main() {
    let _ = Container::new(async_registry! {
        provide(App, async |_: InjectCustom<Opaque>, _: InjectTransient<A>| Ok::<_, InstantiateErrorKind>(A)),
    });
}
