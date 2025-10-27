#[cfg(feature = "async")]
use crate::async_impl;
use crate::{any::TypeInfo, registry, utils::hlist, Registry};

pub trait Merge<T> {
    #[must_use]
    fn merge(self, other: T) -> Self;
}

impl Merge<Registry> for Registry {
    #[inline]
    fn merge(mut self, other: Self) -> Self {
        self.entries.extend(other.entries);
        self
    }
}

impl<H> Merge<H> for Registry
where
    H: hlist::IntoIterator<(TypeInfo, registry::InstantiatorData)>,
{
    #[inline]
    fn merge(mut self, other: H) -> Self {
        self.entries.extend(other.into_iter());
        self
    }
}

#[cfg(feature = "async")]
impl Merge<async_impl::RegistryWithSync> for async_impl::RegistryWithSync {
    #[inline]
    fn merge(mut self, other: Self) -> Self {
        self.registry.entries.extend(other.registry.entries);
        self.sync = self.sync.merge(other.sync);
        self
    }
}

#[cfg(feature = "async")]
impl<H> Merge<H> for async_impl::RegistryWithSync
where
    H: hlist::IntoIterator<(TypeInfo, async_impl::registry::InstantiatorData)>,
{
    #[inline]
    fn merge(mut self, other: H) -> Self {
        self.registry.entries.extend(other.into_iter());
        self
    }
}
