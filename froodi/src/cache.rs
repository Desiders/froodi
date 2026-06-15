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
    #[inline]
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
    pub(crate) fn extend_context(&mut self, context: &Context) {
        self.map
            .extend(context.map.iter().map(|(type_info, value)| (type_info.clone(), value.clone())));
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

#[cfg(test)]
mod tests {
    extern crate std;
    use super::{Cache, Resolved};
    use crate::{any::TypeInfo, utils::thread_safety::RcThreadSafety, Context};

    #[derive(Debug, PartialEq, Eq)]
    struct Foo(u32);

    #[derive(Debug, PartialEq, Eq)]
    struct Bar(u32);

    fn resolved_of<T: Send + Sync + 'static>(value: T) -> Resolved {
        let rc = RcThreadSafety::new(value);
        Resolved {
            type_info: TypeInfo::of::<T>(),
            dependency: rc as _,
        }
    }

    #[test]
    fn new_get_insert_and_replace() {
        let mut cache = Cache::new();
        let ti = TypeInfo::of::<Foo>();

        assert!(cache.get::<Foo>(&ti).is_none());

        let first = RcThreadSafety::new(Foo(1));
        let prev = cache.insert_rc::<Foo>(ti.clone(), first.clone());
        assert!(prev.is_none());

        // get returns the same Arc (pointer identity).
        let got = cache.get::<Foo>(&ti).expect("value must be present after insert");
        assert!(RcThreadSafety::ptr_eq(&first, &got));
        assert_eq!(*got, Foo(1));

        let second = RcThreadSafety::new(Foo(2));
        let returned = cache.insert_rc::<Foo>(ti.clone(), second.clone()).expect("previous value expected");
        assert!(RcThreadSafety::ptr_eq(&first, &returned));
        assert_eq!(*returned, Foo(1));

        let got2 = cache.get::<Foo>(&ti).expect("replacement must be present");
        assert!(RcThreadSafety::ptr_eq(&second, &got2));
        assert_eq!(*got2, Foo(2));
    }

    #[test]
    fn child_clones_map_but_not_resolved() {
        let mut parent = Cache::new();
        let ti = TypeInfo::of::<Foo>();
        let value = RcThreadSafety::new(Foo(7));
        parent.insert_rc::<Foo>(ti.clone(), value.clone());

        parent.push_resolved(resolved_of(Bar(1)));
        assert_eq!(parent.resolved.0.len(), 1);

        let child = parent.child();

        // child() clones the map but resets the resolved set.
        let got = child.get::<Foo>(&ti).expect("child should see parent's cached value");
        assert!(RcThreadSafety::ptr_eq(&value, &got));

        assert_eq!(child.resolved.0.len(), 0);
        assert_eq!(parent.resolved.0.len(), 1);
    }

    #[test]
    fn extend_context_copies_entries() {
        let mut ctx = Context::new();
        assert!(ctx.insert(Bar(5)).is_none());

        let mut cache = Cache::new();
        let ti = TypeInfo::of::<Bar>();
        assert!(cache.get::<Bar>(&ti).is_none());

        cache.extend_context(&ctx);

        let got = cache.get::<Bar>(&ti).expect("context entry must be present after extend_context");
        assert_eq!(*got, Bar(5));
    }

    #[test]
    fn push_and_take_resolved_set() {
        let mut cache = Cache::new();
        assert_eq!(cache.resolved.0.len(), 0);

        cache.push_resolved(resolved_of(Foo(1)));
        cache.push_resolved(resolved_of(Bar(2)));
        assert_eq!(cache.resolved.0.len(), 2);

        let taken = cache.take_resolved_set();
        assert_eq!(taken.0.len(), 2);
        assert_eq!(cache.resolved.0.len(), 0);

        let taken_again = cache.take_resolved_set();
        assert_eq!(taken_again.0.len(), 0);
    }
}
