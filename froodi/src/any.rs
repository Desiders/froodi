use alloc::collections::BTreeMap;
use core::{
    any::{type_name, TypeId},
    cmp::Ordering,
};

use crate::utils::thread_safety::RcAnyThreadSafety;

#[derive(Debug, Clone, Copy)]
pub struct TypeInfo {
    pub name: &'static str,
    pub id: TypeId,
}

impl PartialEq for TypeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for TypeInfo {}

impl PartialOrd for TypeInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TypeInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl TypeInfo {
    #[inline]
    #[must_use]
    #[cfg(const_type_id)]
    pub(crate) const fn new<T: ?Sized + 'static>(name: &'static str) -> Self {
        Self {
            name,
            id: TypeId::of::<T>(),
        }
    }

    #[inline]
    #[must_use]
    #[cfg(not(const_type_id))]
    pub(crate) fn new<T: ?Sized + 'static>(name: &'static str) -> Self {
        Self {
            name,
            id: TypeId::of::<T>(),
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn of<T>() -> Self
    where
        T: ?Sized + 'static,
    {
        Self {
            name: type_name::<T>(),
            id: TypeId::of::<T>(),
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn of_val<T>(_val: &T) -> Self
    where
        T: ?Sized + 'static,
    {
        Self {
            name: type_name::<T>(),
            id: TypeId::of::<T>(),
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn short_name(&self) -> &'static str {
        self.name.rsplit_once("::").map_or(self.name, |(_, name)| name)
    }
}

pub(crate) type Map = BTreeMap<TypeInfo, RcAnyThreadSafety>;
