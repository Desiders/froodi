use alloc::collections::BTreeMap;
use core::any::{type_name, TypeId};

use crate::utils::thread_safety::RcAnyThreadSafety;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TypeInfo {
    pub name: &'static str,
    pub id: TypeId,
}

impl TypeInfo {
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
}

pub(crate) type Map = BTreeMap<TypeInfo, RcAnyThreadSafety>;
