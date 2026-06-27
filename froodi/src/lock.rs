#[cfg(feature = "thread_safe")]
mod sync_impl {
    use alloc::collections::BTreeMap;
    use core::any::TypeId;

    use parking_lot::{Mutex, RwLock};

    use crate::utils::thread_safety::RcThreadSafety;

    #[derive(Clone)]
    pub(crate) struct PerTypeLocks {
        locks: RcThreadSafety<RwLock<BTreeMap<TypeId, RcThreadSafety<Mutex<()>>>>>,
    }

    impl PerTypeLocks {
        #[inline]
        #[must_use]
        fn new() -> Self {
            Self {
                locks: RcThreadSafety::new(RwLock::new(BTreeMap::new())),
            }
        }

        #[inline]
        #[must_use]
        pub(crate) fn get(&self, type_id: TypeId) -> RcThreadSafety<Mutex<()>> {
            if let Some(lock) = self.locks.read().get(&type_id) {
                return lock.clone();
            }

            self.locks
                .write()
                .entry(type_id)
                .or_insert_with(|| RcThreadSafety::new(Mutex::new(())))
                .clone()
        }
    }

    impl Default for PerTypeLocks {
        #[inline]
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(feature = "thread_safe")]
pub(crate) use sync_impl::PerTypeLocks;

#[cfg(feature = "async")]
mod async_impl {
    use alloc::collections::BTreeMap;
    use core::any::TypeId;

    use parking_lot::RwLock;
    use tokio::sync::Mutex;

    use crate::utils::thread_safety::RcThreadSafety;

    #[derive(Clone)]
    pub(crate) struct PerTypeSharedLocks {
        locks: RcThreadSafety<RwLock<BTreeMap<TypeId, RcThreadSafety<Mutex<()>>>>>,
    }

    impl PerTypeSharedLocks {
        #[inline]
        #[must_use]
        fn new() -> Self {
            Self {
                locks: RcThreadSafety::new(RwLock::new(BTreeMap::new())),
            }
        }

        #[inline]
        #[must_use]
        pub(crate) fn get(&self, type_id: TypeId) -> RcThreadSafety<Mutex<()>> {
            if let Some(lock) = self.locks.read().get(&type_id) {
                return lock.clone();
            }

            self.locks
                .write()
                .entry(type_id)
                .or_insert_with(|| RcThreadSafety::new(Mutex::new(())))
                .clone()
        }
    }

    impl Default for PerTypeSharedLocks {
        #[inline]
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(feature = "async")]
pub(crate) use async_impl::PerTypeSharedLocks;
