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

    #[inline]
    #[must_use]
    pub(crate) fn short_name(&self) -> &'static str {
        let bytes = self.name.as_bytes();
        let mut colons = 0;
        let mut i = bytes.len();

        while i >= 2 {
            i -= 1;
            if bytes[i] == b':' && i > 0 && bytes[i - 1] == b':' {
                colons += 1;
                if colons == 2 {
                    return &self.name[i + 1..];
                }
                i -= 1;
            }
        }
        self.name
    }
}

pub(crate) type Map = BTreeMap<TypeInfo, RcAnyThreadSafety>;
