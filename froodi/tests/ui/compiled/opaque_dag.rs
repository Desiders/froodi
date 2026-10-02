use froodi::{
    compiled_registry as registry, instance, Container, DefaultScope::App, DependencyResolver, Inject, InstantiateErrorKind,
    ResolveErrorKind, RuntimeDependency,
};

struct Opaque;

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
    let container = Container::new(registry! {
        provide(App, |_: RuntimeDependency<Opaque>, value: Inject<u32>| Ok::<_, InstantiateErrorKind>(value.0.to_string())),
        provide(App, instance(7u32)),
    });
    assert_eq!(&*container.get::<String>().unwrap(), "7");
}
