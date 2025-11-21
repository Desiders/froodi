use core::iter;
use frunk::{HCons, HNil};

#[cfg(feature = "async")]
use crate::macros_utils::types::RegistryKindOrEntry;
use crate::macros_utils::types::RegistryOrEntry;

pub trait IntoIterator<T> {
    type Iter: Iterator<Item = T>;
    fn into_iter(self) -> Self::Iter;
}

impl<T> IntoIterator<T> for HNil {
    type Iter = iter::Empty<T>;

    #[inline]
    fn into_iter(self) -> Self::Iter {
        iter::empty()
    }
}

impl<H, Tail> IntoIterator<H> for HCons<H, Tail>
where
    Tail: IntoIterator<H>,
{
    type Iter = iter::Chain<iter::Once<H>, Tail::Iter>;

    #[inline]
    fn into_iter(self) -> Self::Iter {
        iter::once(self.head).chain(self.tail.into_iter())
    }
}

impl<Head, Tail> IntoIterator<RegistryOrEntry> for HCons<Head, Tail>
where
    Head: IntoIterator<RegistryOrEntry>,
    Tail: IntoIterator<RegistryOrEntry>,
{
    type Iter = iter::Chain<Head::Iter, Tail::Iter>;

    #[inline]
    fn into_iter(self) -> Self::Iter {
        self.head.into_iter().chain(self.tail.into_iter())
    }
}

#[cfg(feature = "async")]
impl<Head, Tail> IntoIterator<RegistryKindOrEntry> for HCons<Head, Tail>
where
    Head: IntoIterator<RegistryKindOrEntry>,
    Tail: IntoIterator<RegistryKindOrEntry>,
{
    type Iter = iter::Chain<Head::Iter, Tail::Iter>;

    #[inline]
    fn into_iter(self) -> Self::Iter {
        self.head.into_iter().chain(self.tail.into_iter())
    }
}
