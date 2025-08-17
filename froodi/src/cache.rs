use alloc::{boxed::Box, collections::vec_deque::VecDeque, sync::Arc};
use core::{
    any::{Any, TypeId},
    mem,
};

use crate::{any, Context};

#[derive(Clone)]
pub(crate) struct Cache {
    pub(crate) map: Option<Box<any::Map>>,
    pub(crate) resolved: ResolvedSet,
}

impl Cache {
    #[must_use]
    pub fn new() -> Self {
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
        if let (Some(cache), Some(context)) = (&mut self.map, context.map.as_ref()) {
            cache.append(&mut (*context).clone());
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
    pub(crate) fn take_resolved_set(&mut self) -> ResolvedSet {
        mem::take(&mut self.resolved)
    }
}

#[derive(Clone)]
pub(crate) struct Resolved {
    pub(crate) type_id: TypeId,
    pub(crate) dependency: Arc<dyn Any + Send + Sync>,
}

#[derive(Default, Clone)]
pub(crate) struct ResolvedSet(pub(crate) VecDeque<Resolved>);

impl ResolvedSet {
    pub(crate) fn new() -> Self {
        Self(VecDeque::new())
    }

    pub(crate) fn push(&mut self, resolved: Resolved) {
        self.0.push_back(resolved);
    }
}
