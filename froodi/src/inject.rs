use alloc::sync::Arc;

use crate::{async_impl::Container as AsyncContainer, dependency_resolver::DependencyResolver, Container, ResolveErrorKind};

pub struct Inject<Dep, const PREFER_SYNC_OVER_ASYNC: bool = true>(pub Arc<Dep>);

impl<Dep: Send + Sync + 'static> DependencyResolver for Inject<Dep> {
    type Error = ResolveErrorKind;

    fn resolve(container: &Container) -> Result<Self, Self::Error> {
        container.get().map(Self)
    }

    #[cfg(feature = "async")]
    async fn resolve_async(container: &AsyncContainer) -> Result<Self, Self::Error> {
        container.get().await.map(Self)
    }
}

pub struct InjectTransient<Dep, const PREFER_SYNC_OVER_ASYNC: bool = true>(pub Dep);

impl<Dep: 'static> DependencyResolver for InjectTransient<Dep> {
    type Error = ResolveErrorKind;

    fn resolve(container: &Container) -> Result<Self, Self::Error> {
        container.get_transient().map(Self)
    }

    #[cfg(feature = "async")]
    async fn resolve_async(container: &AsyncContainer) -> Result<Self, Self::Error> {
        container.get_transient().await.map(Self)
    }
}
