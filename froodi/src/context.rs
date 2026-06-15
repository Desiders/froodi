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
            .insert(any::TypeInfo::of::<T>(), RcThreadSafety::new(value))
            .and_then(|boxed| boxed.downcast().ok())
    }

    #[inline]
    pub fn insert_rc<T: SendSafety + SyncSafety + 'static>(&mut self, value: RcThreadSafety<T>) -> Option<RcThreadSafety<T>> {
        self.map
            .insert(any::TypeInfo::of::<T>(), value)
            .and_then(|boxed| boxed.downcast().ok())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::Context;
    use crate::utils::thread_safety::RcThreadSafety;

    #[derive(Debug, PartialEq, Eq)]
    struct Foo(u32);

    #[derive(Debug, PartialEq, Eq)]
    struct Bar(u32);

    #[test]
    fn new_and_default_are_empty() {
        assert_eq!(Context::new().map.len(), 0);
        assert_eq!(Context::default().map.len(), 0);
    }

    #[test]
    fn insert_returns_previous_and_keeps_one_entry() {
        let mut ctx = Context::new();

        assert!(ctx.insert(Foo(1)).is_none());
        assert_eq!(ctx.map.len(), 1);

        let prev = ctx.insert(Foo(2)).expect("previous value expected");
        assert_eq!(*prev, Foo(1));
        assert_eq!(ctx.map.len(), 1);
    }

    #[test]
    fn insert_rc_behaves_like_insert() {
        let mut ctx = Context::new();

        let first = RcThreadSafety::new(Foo(10));
        assert!(ctx.insert_rc::<Foo>(first.clone()).is_none());
        assert_eq!(ctx.map.len(), 1);

        // Replacing returns the previous Arc (pointer identity preserved).
        let second = RcThreadSafety::new(Foo(20));
        let prev = ctx.insert_rc::<Foo>(second).expect("previous value expected");
        assert!(RcThreadSafety::ptr_eq(&first, &prev));
        assert_eq!(*prev, Foo(10));
        assert_eq!(ctx.map.len(), 1);
    }

    #[test]
    fn different_types_grow_the_map() {
        let mut ctx = Context::new();
        assert!(ctx.insert(Foo(1)).is_none());
        assert!(ctx.insert(Bar(2)).is_none());
        assert_eq!(ctx.map.len(), 2);
    }
}
