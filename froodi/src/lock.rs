#![allow(dead_code)]

use ahash::AHasher;
use core::{
    array,
    hash::{Hash, Hasher as _},
};
use parking_lot::Mutex;

use crate::utils::thread_safety::RcThreadSafety;

#[allow(clippy::cast_possible_truncation)]
fn stripe_index<const N: usize>(val: impl Hash) -> usize {
    let mut hasher = AHasher::default();
    val.hash(&mut hasher);

    hasher.finish() as usize % N
}

#[derive(Clone)]
pub(crate) struct StripedLocks<const N: usize> {
    stripes: RcThreadSafety<[Mutex<()>; N]>,
}

impl<const N: usize> StripedLocks<N> {
    #[inline]
    #[must_use]
    fn new() -> Self {
        Self {
            stripes: RcThreadSafety::new(array::from_fn(|_| Mutex::new(()))),
        }
    }
}

impl<const N: usize> StripedLocks<N> {
    #[inline]
    #[must_use]
    pub(crate) fn get(&self, val: impl Hash) -> &Mutex<()> {
        &self.stripes[stripe_index::<N>(val)]
    }
}

impl<const N: usize> Default for StripedLocks<N> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "async")]
mod async_impl {
    use tokio::sync::Mutex;

    use super::{array, stripe_index, Hash, RcThreadSafety};

    #[derive(Clone)]
    pub(crate) struct StripedSharedLocks<const N: usize> {
        stripes: RcThreadSafety<[Mutex<()>; N]>,
    }

    impl<const N: usize> StripedSharedLocks<N> {
        #[inline]
        #[must_use]
        fn new() -> Self {
            Self {
                stripes: RcThreadSafety::new(array::from_fn(|_| Mutex::new(()))),
            }
        }
    }

    impl<const N: usize> StripedSharedLocks<N> {
        #[inline]
        #[must_use]
        pub(crate) fn get(&self, val: impl Hash) -> &Mutex<()> {
            &self.stripes[stripe_index::<N>(val)]
        }
    }

    impl<const N: usize> Default for StripedSharedLocks<N> {
        #[inline]
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(feature = "async")]
pub(crate) use async_impl::StripedSharedLocks;
