use alloc::{boxed::Box, collections::vec_deque::VecDeque, sync::Arc};
use core::any::{Any, TypeId};

use crate::{any, Context};

#[derive(Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub(crate) struct Cache {
    pub(crate) map: Option<Box<any::Map>>,
    resolved: ResolvedSet,
}

#[cfg(feature = "eq")]
impl PartialEq for Cache {
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
impl Eq for Cache {}

impl Cache {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            map: None,
            resolved: ResolvedSet::new(),
        }
    }

    #[inline]
    pub(crate) fn insert_rc<T: Send + Sync + 'static>(&mut self, value: Arc<T>) -> Option<Arc<T>> {
        self.map
            .get_or_insert_with(Box::default)
            .insert(TypeId::of::<T>(), value)
            .and_then(|boxed| boxed.downcast().ok())
    }

    #[inline]
    pub(crate) fn append_context(&mut self, context: &Context) {
        match (&mut self.map, context.map.as_ref()) {
            (Some(cache), Some(context)) => cache.append(&mut (*context).clone()),
            _ => {}
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn child(&self) -> Self {
        Self {
            map: self.map.clone(),
            resolved: ResolvedSet::new(),
        }
    }

    #[must_use]
    pub(crate) fn get<T: Send + Sync + 'static>(&self, type_id: &TypeId) -> Option<Arc<T>> {
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

#[derive(Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub(crate) struct Resolved {
    pub(crate) type_id: TypeId,
    pub(crate) dependency: Arc<dyn Any + Send + Sync>,
}

#[derive(Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub(crate) struct ResolvedSet(pub(crate) VecDeque<Resolved>);

impl ResolvedSet {
    pub(crate) const fn new() -> Self {
        Self(VecDeque::new())
    }

    pub(crate) fn push(&mut self, resolved: Resolved) {
        self.0.push_back(resolved);
    }
}
