//! Locking primitives used by the container.
//!
//! Non-thread-safe builds use `RefCell`. Thread-safe builds select a lock backend in this order:
//! `lock-parking-lot`, `lock-spin`, then `std::sync`.
//!
//! `no_std + thread_safe` requires an explicit no_std-compatible backend.
//!
//! Per-type locks serialize concurrent instantiation of the same cached type so it is built only once.
//! Sync containers use the selected backend; async containers use `tokio::sync::Mutex`.

#[cfg(all(feature = "thread_safe", not(feature = "std"), not(feature = "lock-spin")))]
compile_error!(
    "`thread_safe` in `no_std` mode requires an explicit no_std-compatible lock backend; \
     enable a supported lock backend feature"
);

#[cfg(feature = "lock-parking-lot")]
mod backend {
    pub(crate) use parking_lot::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
}

#[cfg(all(not(feature = "lock-parking-lot"), feature = "lock-spin"))]
mod backend {
    pub(crate) use spin::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
}

#[cfg(all(
    feature = "thread_safe",
    feature = "std",
    not(feature = "lock-parking-lot"),
    not(feature = "lock-spin"),
))]
mod backend {
    use std::sync;

    pub(crate) type RwLockReadGuard<'a, T> = sync::RwLockReadGuard<'a, T>;
    pub(crate) type RwLockWriteGuard<'a, T> = sync::RwLockWriteGuard<'a, T>;

    pub(crate) struct RwLock<T>(sync::RwLock<T>);

    impl<T> RwLock<T> {
        #[inline]
        pub(crate) const fn new(value: T) -> Self {
            Self(sync::RwLock::new(value))
        }

        #[inline]
        pub(crate) fn read(&self) -> RwLockReadGuard<'_, T> {
            self.0.read().unwrap_or_else(sync::PoisonError::into_inner)
        }

        #[inline]
        pub(crate) fn write(&self) -> RwLockWriteGuard<'_, T> {
            self.0.write().unwrap_or_else(sync::PoisonError::into_inner)
        }

        #[allow(unused)]
        #[inline]
        pub(crate) fn get_mut(&mut self) -> &mut T {
            self.0.get_mut().unwrap_or_else(sync::PoisonError::into_inner)
        }
    }

    pub(crate) struct Mutex<T>(sync::Mutex<T>);

    impl<T> Mutex<T> {
        #[inline]
        pub(crate) const fn new(value: T) -> Self {
            Self(sync::Mutex::new(value))
        }

        #[inline]
        pub(crate) fn lock(&self) -> sync::MutexGuard<'_, T> {
            self.0.lock().unwrap_or_else(sync::PoisonError::into_inner)
        }
    }

    impl<T: Default> Default for Mutex<T> {
        #[inline]
        fn default() -> Self {
            Self::new(T::default())
        }
    }
}

mod generic {
    #[cfg(feature = "thread_safe")]
    use super::backend::{RwLock, RwLockReadGuard, RwLockWriteGuard};
    #[cfg(not(feature = "thread_safe"))]
    use core::cell::{Ref, RefCell, RefMut};

    /// Interior-mutability primitive for synchronously accessed container state.
    ///
    /// In non-thread-safe builds this is backed by `RefCell`.
    ///
    /// In thread-safe builds it is backed by the selected synchronization backend.
    pub(crate) struct LocalLock<T> {
        #[cfg(feature = "thread_safe")]
        inner: RwLock<T>,
        #[cfg(not(feature = "thread_safe"))]
        inner: RefCell<T>,
    }

    impl<T> LocalLock<T> {
        #[inline]
        pub(crate) const fn new(value: T) -> Self {
            Self {
                #[cfg(feature = "thread_safe")]
                inner: RwLock::new(value),
                #[cfg(not(feature = "thread_safe"))]
                inner: RefCell::new(value),
            }
        }

        #[cfg(feature = "thread_safe")]
        #[inline]
        pub(crate) fn read(&self) -> RwLockReadGuard<'_, T> {
            self.inner.read()
        }

        #[cfg(not(feature = "thread_safe"))]
        #[inline]
        pub(crate) fn read(&self) -> Ref<'_, T> {
            self.inner.borrow()
        }

        #[cfg(feature = "thread_safe")]
        #[inline]
        pub(crate) fn write(&self) -> RwLockWriteGuard<'_, T> {
            self.inner.write()
        }

        #[cfg(not(feature = "thread_safe"))]
        #[inline]
        pub(crate) fn write(&self) -> RefMut<'_, T> {
            self.inner.borrow_mut()
        }

        #[allow(unused)]
        #[inline]
        pub(crate) fn get_mut(&mut self) -> &mut T {
            self.inner.get_mut()
        }
    }

    impl<T: Default> Default for LocalLock<T> {
        #[inline]
        fn default() -> Self {
            Self::new(T::default())
        }
    }
}

#[cfg(any(feature = "thread_safe", feature = "async"))]
mod generic_per_type {
    use alloc::collections::BTreeMap;
    use core::any::TypeId;

    use super::generic::LocalLock;
    use crate::utils::thread_safety::RcThreadSafety;

    /// A `TypeId`-keyed registry of lazily-created locks of type `M`.
    ///
    /// Cloning shares the same registry.
    pub(crate) struct TypeKeyedLocks<M> {
        locks: RcThreadSafety<LocalLock<BTreeMap<TypeId, RcThreadSafety<M>>>>,
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
                locks: RcThreadSafety::new(LocalLock::new(BTreeMap::new())),
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

pub(crate) use generic::LocalLock;

/// Synchronous per-type instantiation locks using the selected synchronization backend.
///
/// This deliberately uses the same backend as [`LocalLock`], so choosing an override replaces all
/// synchronous locking primitives rather than only the container-state lock.
#[cfg(feature = "thread_safe")]
pub(crate) type PerTypeSyncLocks = generic_per_type::TypeKeyedLocks<backend::Mutex<()>>;

/// Async per-type instantiation locks (`tokio::sync::Mutex`).
#[cfg(feature = "async")]
pub(crate) type PerTypeAsyncLocks = generic_per_type::TypeKeyedLocks<tokio::sync::Mutex<()>>;
