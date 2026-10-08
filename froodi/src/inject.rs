use core::slice::Iter;

#[cfg(feature = "async")]
use crate::async_impl::Container as AsyncContainer;
#[cfg(feature = "async")]
use crate::registry::Selected;
use crate::{
    any::TypeInfo,
    dependency_resolver::DependencyResolver,
    registry::{RegistrationId, Selection},
    utils::thread_safety::{RcThreadSafety, SendSafety, SyncSafety},
    Container, ResolveErrorKind,
};

pub struct Inject<Dep, const PREFER_SYNC_OVER_ASYNC: bool = true>(pub RcThreadSafety<Dep>);

impl<Dep: SendSafety + SyncSafety + 'static> DependencyResolver for Inject<Dep> {
    type Error = ResolveErrorKind;

    #[inline]
    fn resolve(container: &Container) -> Result<Self, Self::Error> {
        container.get().map(Self)
    }

    #[inline]
    fn resolve_linked(container: &Container, edges: &mut Iter<'_, RegistrationId>) -> Result<Self, Self::Error> {
        let id = edges.next().expect("one linked edge per injection");
        container
            .get_selected(Selection::Indexed(container.inner.registry.indexed[id.index()].1.sync()))
            .map(Self)
    }

    #[inline]
    #[cfg(feature = "async")]
    async fn resolve_async(container: &AsyncContainer) -> Result<Self, Self::Error> {
        container.get().await.map(Self)
    }

    #[inline]
    #[cfg(feature = "async")]
    async fn resolve_async_linked(container: &AsyncContainer, edges: &mut Iter<'_, RegistrationId>) -> Result<Self, Self::Error> {
        let id = *edges.next().expect("one linked edge per injection");
        match &container.inner.registry.indexed[id.index()].1.selected {
            Selected::Sync(data) => container.sync.get_selected(Selection::Indexed(Some(data))).map(Self),
            Selected::Async(data) => container.get_selected(Selection::Indexed(Some(data))).await.map(Self),
            Selected::Missing => container.get_selected(Selection::Indexed(None)).await.map(Self),
        }
    }

    #[inline]
    fn type_info() -> TypeInfo {
        TypeInfo::of::<Dep>()
    }
}

pub struct InjectTransient<Dep, const PREFER_SYNC_OVER_ASYNC: bool = true>(pub Dep);

impl<Dep: 'static> DependencyResolver for InjectTransient<Dep> {
    type Error = ResolveErrorKind;

    #[inline]
    fn resolve(container: &Container) -> Result<Self, Self::Error> {
        container.get_transient().map(Self)
    }

    #[inline]
    fn resolve_linked(container: &Container, edges: &mut Iter<'_, RegistrationId>) -> Result<Self, Self::Error> {
        let id = edges.next().expect("one linked edge per injection");
        container
            .get_transient_selected(Selection::Indexed(container.inner.registry.indexed[id.index()].1.sync()))
            .map(Self)
    }

    #[inline]
    #[cfg(feature = "async")]
    async fn resolve_async(container: &AsyncContainer) -> Result<Self, Self::Error> {
        container.get_transient().await.map(Self)
    }

    #[inline]
    #[cfg(feature = "async")]
    async fn resolve_async_linked(container: &AsyncContainer, edges: &mut Iter<'_, RegistrationId>) -> Result<Self, Self::Error> {
        let id = *edges.next().expect("one linked edge per injection");
        match &container.inner.registry.indexed[id.index()].1.selected {
            Selected::Sync(data) => container.sync.get_transient_selected(Selection::Indexed(Some(data))).map(Self),
            Selected::Async(data) => container.get_transient_selected(Selection::Indexed(Some(data))).await.map(Self),
            Selected::Missing => container.get_transient_selected(Selection::Indexed(None)).await.map(Self),
        }
    }

    #[inline]
    fn type_info() -> TypeInfo {
        TypeInfo::of::<Dep>()
    }
}

/// Resolves a custom [`DependencyResolver`] without a statically linked edge.
///
/// The resolver runs at construction time and is responsible for its own lookups.
pub struct InjectCustom<T>(pub T);

impl<T: DependencyResolver> DependencyResolver for InjectCustom<T> {
    type Error = T::Error;

    fn resolve(container: &Container) -> Result<Self, Self::Error> {
        T::resolve(container).map(Self)
    }

    #[cfg(feature = "async")]
    async fn resolve_async(container: &AsyncContainer) -> Result<Self, Self::Error> {
        T::resolve_async(container).await.map(Self)
    }
}
