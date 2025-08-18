use alloc::sync::Arc;
use core::any::TypeId;

use crate::any;

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
    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T) -> Option<Arc<T>> {
        self.map
            .insert(TypeId::of::<T>(), Arc::new(value))
            .and_then(|boxed| boxed.downcast().ok())
    }

    #[inline]
    pub fn insert_rc<T: Send + Sync + 'static>(&mut self, value: Arc<T>) -> Option<Arc<T>> {
        self.map.insert(TypeId::of::<T>(), value).and_then(|boxed| boxed.downcast().ok())
    }
}
