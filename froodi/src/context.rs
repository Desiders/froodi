use core::any::TypeId;

use crate::{
    any,
    utils::thread_safety::{RcThreadSafety, SendSafety, SyncSafety},
};

#[derive(Clone)]
pub struct Context {
    pub(crate) map: any::Map,
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self { map: any::Map::new() }
    }

    #[inline]
    pub fn insert<T: SendSafety + SyncSafety + 'static>(&mut self, value: T) -> Option<RcThreadSafety<T>> {
        self.map
            .insert(TypeId::of::<T>(), RcThreadSafety::new(value))
            .and_then(|boxed| boxed.downcast().ok())
    }

    #[inline]
    pub fn insert_rc<T: SendSafety + SyncSafety + 'static>(&mut self, value: RcThreadSafety<T>) -> Option<RcThreadSafety<T>> {
        self.map.insert(TypeId::of::<T>(), value).and_then(|boxed| boxed.downcast().ok())
    }
}
