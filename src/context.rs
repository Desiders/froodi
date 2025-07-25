use alloc::{boxed::Box, sync::Arc};
use core::any::TypeId;

use crate::any;

#[derive(Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Context {
    pub(crate) map: Option<Box<any::Map>>,
}

#[cfg(feature = "eq")]
impl PartialEq for Context {
    fn eq(&self, other: &Self) -> bool {
        match (&self.map, &other.map) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                if a.len() != b.len() {
                    return false;
                }
                for ((k_a, v_a), (k_b, v_b)) in a.iter().zip(b.iter()) {
                    if k_a != k_b || v_a.type_id() != v_b.type_id() {
                        return false;
                    }
                }
                true
            }
            _ => false,
        }
    }
}

#[cfg(feature = "eq")]
impl Eq for Context {}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { map: None }
    }

    #[inline]
    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T) -> Option<Arc<T>> {
        self.map
            .get_or_insert_with(Box::default)
            .insert(TypeId::of::<T>(), Arc::new(value))
            .and_then(|boxed| boxed.downcast().ok())
    }

    #[inline]
    pub fn insert_rc<T: Send + Sync + 'static>(&mut self, value: Arc<T>) -> Option<Arc<T>> {
        self.map
            .get_or_insert_with(Box::default)
            .insert(TypeId::of::<T>(), value)
            .and_then(|boxed| boxed.downcast().ok())
    }
}
