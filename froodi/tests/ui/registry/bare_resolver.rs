use froodi::{registry, Container, DefaultScope::App, DependencyResolver, InstantiateErrorKind, ResolveErrorKind};

struct Custom;

impl DependencyResolver for Custom {
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
    let _ = Container::new(registry! {
        provide(App, |_: Custom| Ok::<_, InstantiateErrorKind>(7u32)),
    });
}
