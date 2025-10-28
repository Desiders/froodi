use core::iter;
use frunk::{HCons, HNil};

#[cfg(feature = "async")]
use crate::macros_utils::types::RegistryKindOrEntry;
use crate::macros_utils::types::RegistryOrEntry;

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

impl<Head, Tail> IntoIterator<RegistryOrEntry> for HCons<Head, Tail>
where
    Head: IntoIterator<RegistryOrEntry>,
    Tail: IntoIterator<RegistryOrEntry>,
{
    #[inline]
    fn into_iter(self) -> impl Iterator<Item = RegistryOrEntry> {
        self.head.into_iter().chain(self.tail.into_iter())
    }
}

#[cfg(feature = "async")]
impl<Head, Tail> IntoIterator<RegistryKindOrEntry> for HCons<Head, Tail>
where
    Head: IntoIterator<RegistryKindOrEntry>,
    Tail: IntoIterator<RegistryKindOrEntry>,
{
    #[inline]
    fn into_iter(self) -> impl Iterator<Item = RegistryKindOrEntry> {
        self.head.into_iter().chain(self.tail.into_iter())
    }
}
