#[cfg(feature = "async")]
use crate::async_impl::Container as AsyncContainer;
#[cfg(all(feature = "compiled", feature = "async"))]
use crate::compiled::async_impl::Selected;
use crate::{
    any::TypeInfo,
    dependency_resolver::DependencyResolver,
    utils::thread_safety::{RcThreadSafety, SendSafety, SyncSafety},
    Container, ResolveErrorKind,
};
#[cfg(feature = "compiled")]
use crate::{compiled::RegistrationId, registry::Selection};
#[cfg(feature = "compiled")]
use core::slice::Iter;

pub struct Inject<Dep, const PREFER_SYNC_OVER_ASYNC: bool = true>(pub RcThreadSafety<Dep>);

impl<Dep: SendSafety + SyncSafety + 'static> DependencyResolver for Inject<Dep> {
    type Error = ResolveErrorKind;

    #[inline]
    fn resolve(container: &Container) -> Result<Self, Self::Error> {
        container.get().map(Self)
    }

    #[inline]
    #[cfg(feature = "compiled")]
    fn resolve_compiled(container: &Container, edges: &mut Iter<'_, RegistrationId>) -> Result<Self, Self::Error> {
        let id = edges.next().expect("one compiled edge per injection");
        container
            .get_selected(Selection::Indexed(container.inner.registry.indexed[id.index()].1.as_ref()))
            .map(Self)
    }

    #[inline]
    #[cfg(feature = "async")]
    async fn resolve_async(container: &AsyncContainer) -> Result<Self, Self::Error> {
        container.get().await.map(Self)
    }

    #[inline]
    #[cfg(all(feature = "compiled", feature = "async"))]
    async fn resolve_async_compiled(container: &AsyncContainer, edges: &mut Iter<'_, RegistrationId>) -> Result<Self, Self::Error> {
        let id = *edges.next().expect("one compiled edge per injection");
        match &container.inner.registry.indexed[id.index()].1 {
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
    #[cfg(feature = "compiled")]
    fn resolve_compiled(container: &Container, edges: &mut Iter<'_, RegistrationId>) -> Result<Self, Self::Error> {
        let id = edges.next().expect("one compiled edge per injection");
        container
            .get_transient_selected(Selection::Indexed(container.inner.registry.indexed[id.index()].1.as_ref()))
            .map(Self)
    }

    #[inline]
    #[cfg(feature = "async")]
    async fn resolve_async(container: &AsyncContainer) -> Result<Self, Self::Error> {
        container.get_transient().await.map(Self)
    }

    #[inline]
    #[cfg(all(feature = "compiled", feature = "async"))]
    async fn resolve_async_compiled(container: &AsyncContainer, edges: &mut Iter<'_, RegistrationId>) -> Result<Self, Self::Error> {
        let id = *edges.next().expect("one compiled edge per injection");
        match &container.inner.registry.indexed[id.index()].1 {
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
