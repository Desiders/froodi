use alloc::collections::vec_deque::VecDeque;
use core::mem;

use crate::{
    any::{Map, TypeInfo},
    utils::thread_safety::{RcAnyThreadSafety, RcThreadSafety, SendSafety, SyncSafety},
    Context,
};

#[derive(Clone)]
pub(crate) struct Cache {
    pub(crate) map: Map,
    pub(crate) resolved: ResolvedSet,
}

impl Cache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: Map::new(),
            resolved: ResolvedSet::new(),
        }
    }

    #[inline]
    pub(crate) fn insert_rc<T: SendSafety + SyncSafety + 'static>(
        &mut self,
        type_info: TypeInfo,
        value: RcThreadSafety<T>,
    ) -> Option<RcThreadSafety<T>> {
        self.map.insert(type_info, value).and_then(|boxed| boxed.downcast().ok())
    }

    #[inline]
    pub(crate) fn append_context(&mut self, context: &mut Context) {
        self.map.append(&mut context.map);
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
    pub(crate) fn get<T: SendSafety + SyncSafety + 'static>(&self, type_info: &TypeInfo) -> Option<RcThreadSafety<T>> {
        self.map.get(type_info).and_then(|boxed| boxed.clone().downcast().ok())
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
    pub(crate) type_info: TypeInfo,
    pub(crate) dependency: RcAnyThreadSafety,
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
