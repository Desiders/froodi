use core::iter;
use frunk::{HCons, HNil};

#[cfg(feature = "async")]
use crate::async_impl;
use crate::{any::TypeInfo, registry};

pub trait IntoIterator<T> {
    fn into_iter(self) -> impl Iterator<Item = T>;
}

impl<T> IntoIterator<T> for HNil {
    #[inline]
    fn into_iter(self) -> impl Iterator<Item = T> {
        iter::empty()
    }
}

impl<H, Tail> IntoIterator<H> for HCons<H, Tail>
where
    Tail: IntoIterator<H>,
{
    #[inline]
    fn into_iter(self) -> impl Iterator<Item = H> {
        iter::once(self.head).chain(self.tail.into_iter())
    }
}

impl<Head, Tail> IntoIterator<(TypeInfo, registry::InstantiatorData)> for HCons<Head, Tail>
where
    Head: IntoIterator<(TypeInfo, registry::InstantiatorData)>,
    Tail: IntoIterator<(TypeInfo, registry::InstantiatorData)>,
{
    #[inline]
    fn into_iter(self) -> impl Iterator<Item = (TypeInfo, registry::InstantiatorData)> {
        self.head.into_iter().chain(self.tail.into_iter())
    }
}

impl<Tail> IntoIterator<(TypeInfo, registry::InstantiatorData)> for HCons<registry::Registry, Tail>
where
    Tail: IntoIterator<(TypeInfo, registry::InstantiatorData)>,
{
    #[inline]
    fn into_iter(self) -> impl Iterator<Item = (TypeInfo, registry::InstantiatorData)> {
        self.head.entries.into_iter().chain(self.tail.into_iter())
    }
}

#[cfg(feature = "async")]
impl<Head, Tail> IntoIterator<(TypeInfo, async_impl::registry::InstantiatorData)> for HCons<Head, Tail>
where
    Head: IntoIterator<(TypeInfo, async_impl::registry::InstantiatorData)>,
    Tail: IntoIterator<(TypeInfo, async_impl::registry::InstantiatorData)>,
{
    #[inline]
    fn into_iter(self) -> impl Iterator<Item = (TypeInfo, async_impl::registry::InstantiatorData)> {
        self.head.into_iter().chain(self.tail.into_iter())
    }
}

#[cfg(feature = "async")]
impl<Tail> IntoIterator<(TypeInfo, async_impl::registry::InstantiatorData)> for HCons<async_impl::RegistryWithSync, Tail>
where
    Tail: IntoIterator<(TypeInfo, async_impl::registry::InstantiatorData)>,
{
    #[inline]
    fn into_iter(self) -> impl Iterator<Item = (TypeInfo, async_impl::registry::InstantiatorData)> {
        self.head.registry.entries.into_iter().chain(self.tail.into_iter())
    }
}
