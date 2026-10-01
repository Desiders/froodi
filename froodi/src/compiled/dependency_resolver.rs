#[cfg(feature = "async")]
use crate::async_impl::Container as AsyncContainer;
use crate::{Container, DependencyResolver};

/// Resolves a custom parameter without a statically linked dependency edge
pub struct RuntimeDependency<T>(pub T);

impl<T: DependencyResolver> DependencyResolver for RuntimeDependency<T> {
    type Error = T::Error;

    fn resolve(container: &Container) -> Result<Self, Self::Error> {
        T::resolve(container).map(Self)
    }

    #[cfg(feature = "async")]
    async fn resolve_async(container: &AsyncContainer) -> Result<Self, Self::Error> {
        T::resolve_async(container).await.map(Self)
    }
}
