//! Per-type instantiation locks.
//!
//! These serialize concurrent instantiation of the *same* type at its owning scope so a cached
//! singleton is built exactly once (the resolution path takes the lock, then re-checks the cache).
//! A lock is only ever inserted the first time a given registered type is instantiated, so the map
//! is bounded by the number of distinct instantiated types — not a per-request leak — and is
//! intentionally not pruned: doing so on the hot `close()` path would add write-lock contention on
//! the shared map for negligible gain.
//!
//! The sync container uses `parking_lot::Mutex` and the async container uses `tokio::sync::Mutex`;
//! both share the [`generic::TypeKeyedLocks`] map below (the primitive only differs at the call
//! site, where the sync side `lock()`s and the async side `lock().await`s).

#[cfg(any(feature = "thread_safe", feature = "async"))]
mod generic {
    use alloc::collections::BTreeMap;
    use core::any::TypeId;

    use parking_lot::RwLock;

    use crate::utils::thread_safety::RcThreadSafety;

    /// A `TypeId`-keyed registry of lazily-created locks of type `M`. Cloning shares the same map.
    pub(crate) struct TypeKeyedLocks<M> {
        locks: RcThreadSafety<RwLock<BTreeMap<TypeId, RcThreadSafety<M>>>>,
    }

    impl<M> Clone for TypeKeyedLocks<M> {
        #[inline]
        fn clone(&self) -> Self {
            Self { locks: self.locks.clone() }
        }
    }

    impl<M> Default for TypeKeyedLocks<M> {
        #[inline]
        fn default() -> Self {
            Self {
                locks: RcThreadSafety::new(RwLock::new(BTreeMap::new())),
            }
        }
    }

    impl<M: Default> TypeKeyedLocks<M> {
        #[inline]
        #[must_use]
        pub(crate) fn get(&self, type_id: TypeId) -> RcThreadSafety<M> {
            if let Some(lock) = self.locks.read().get(&type_id) {
                return lock.clone();
            }

            self.locks
                .write()
                .entry(type_id)
                .or_insert_with(|| RcThreadSafety::new(M::default()))
                .clone()
        }
    }
}

/// Synchronous per-type instantiation locks (`parking_lot::Mutex`). See the module docs.
#[cfg(feature = "thread_safe")]
pub(crate) type PerTypeLocks = generic::TypeKeyedLocks<parking_lot::Mutex<()>>;

/// Async per-type instantiation locks (`tokio::sync::Mutex`). See the module docs.
#[cfg(feature = "async")]
pub(crate) type PerTypeSharedLocks = generic::TypeKeyedLocks<tokio::sync::Mutex<()>>;
