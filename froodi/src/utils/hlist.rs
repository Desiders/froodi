use core::{any::TypeId, iter};
use frunk::{HCons, HNil};

#[cfg(feature = "async")]
use crate::async_impl;
use crate::registry;

pub trait IntoIterator<T> {
    fn into_iter(self) -> impl Iterator<Item = T>;
}

impl<T> IntoIterator<T> for HNil {
    fn into_iter(self) -> impl Iterator<Item = T> {
        iter::empty()
    }
}

impl<T, Tail> IntoIterator<T> for HCons<T, Tail>
where
    Tail: IntoIterator<T>,
{
    fn into_iter(self) -> impl Iterator<Item = T> {
        iter::once(self.head).chain(self.tail.into_iter())
    }
}

impl<Head, Tail> IntoIterator<(TypeId, registry::InstantiatorData)> for HCons<Head, Tail>
where
    Head: IntoIterator<(TypeId, registry::InstantiatorData)>,
    Tail: IntoIterator<(TypeId, registry::InstantiatorData)>,
{
    fn into_iter(self) -> impl Iterator<Item = (TypeId, registry::InstantiatorData)> {
        self.head.into_iter().chain(self.tail.into_iter())
    }
}

#[cfg(feature = "async")]
impl<Head, Tail> IntoIterator<(TypeId, async_impl::registry::InstantiatorData)> for HCons<Head, Tail>
where
    Head: IntoIterator<(TypeId, async_impl::registry::InstantiatorData)>,
    Tail: IntoIterator<(TypeId, async_impl::registry::InstantiatorData)>,
{
    fn into_iter(self) -> impl Iterator<Item = (TypeId, async_impl::registry::InstantiatorData)> {
        self.head.into_iter().chain(self.tail.into_iter())
    }
}
