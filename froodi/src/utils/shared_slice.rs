use alloc::vec::Vec;
use core::ops::Deref;

use super::thread_safety::RcThreadSafety;

pub(crate) struct SharedSlice<T>(Option<RcThreadSafety<[T]>>);

impl<T> SharedSlice<T> {
    pub(crate) const fn new() -> Self {
        Self(None)
    }
}

impl<T> Clone for SharedSlice<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> From<Vec<T>> for SharedSlice<T> {
    fn from(values: Vec<T>) -> Self {
        Self((!values.is_empty()).then(|| values.into()))
    }
}

impl<T> Deref for SharedSlice<T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0.as_deref().unwrap_or(&[])
    }
}
