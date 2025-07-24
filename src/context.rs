use alloc::{boxed::Box, rc::Rc};
use core::any::TypeId;

use crate::dependency_resolver::{Resolved, ResolvedSet};

#[derive(Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Context {
    map: Option<Box<any::Map>>,
    resolved: ResolvedSet,
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
    #[must_use]
    pub const fn new() -> Self {
        Self {
            map: None,
            resolved: ResolvedSet::new(),
        }
    }

    #[inline]
    pub fn insert<T: 'static>(&mut self, value: T) -> Option<Rc<T>> {
        self.map
            .get_or_insert_with(Box::default)
            .insert(TypeId::of::<T>(), Rc::new(value))
            .and_then(|boxed| boxed.downcast().ok())
    }

    #[inline]
    pub fn insert_rc<T: 'static>(&mut self, value: Rc<T>) -> Option<Rc<T>> {
        self.map
            .get_or_insert_with(Box::default)
            .insert(TypeId::of::<T>(), value)
            .and_then(|boxed| boxed.downcast().ok())
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.as_ref().is_none_or(|map| map.is_empty())
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.as_ref().map_or(0, |map| map.len())
    }
}

impl Context {
    #[inline]
    #[must_use]
    pub(crate) fn child(&self) -> Self {
        Self {
            map: self.map.clone(),
            resolved: ResolvedSet::new(),
        }
    }

    #[must_use]
    pub(crate) fn get<T: 'static>(&self, type_id: &TypeId) -> Option<Rc<T>> {
        self.map
            .as_ref()
            .and_then(|map| map.get(type_id))
            .and_then(|boxed| boxed.clone().downcast().ok())
    }

    #[inline]
    pub(crate) fn push_resolved(&mut self, resolved: Resolved) {
        self.resolved.push(resolved);
    }

    #[inline]
    #[must_use]
    #[cfg(test)]
    pub(crate) const fn get_resolved_set(&self) -> &ResolvedSet {
        &self.resolved
    }

    #[inline]
    #[must_use]
    pub(crate) const fn get_resolved_set_mut(&mut self) -> &mut ResolvedSet {
        &mut self.resolved
    }
}

mod any {
    use alloc::{collections::BTreeMap, rc::Rc};
    use core::any::{Any, TypeId};

    pub(super) type Map = BTreeMap<TypeId, Rc<dyn Any>>;
}
