use super::{dense_per_type::LockRef, TypeKeyedLocks};
use crate::{any::TypeInfo, registry::RegistrationId};
use core::{
    any::TypeId,
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};

struct First;
struct Second;

#[test]
fn dense_locks_remain_lazy_and_shared_without_per_get_arc() {
    static INITIALIZATIONS: AtomicUsize = AtomicUsize::new(0);

    struct Lock(u8);

    impl Default for Lock {
        fn default() -> Self {
            INITIALIZATIONS.fetch_add(1, Ordering::Relaxed);
            Self(0)
        }
    }

    let locks = TypeKeyedLocks::<Lock>::with_indexed_keys(2, [TypeId::of::<First>(), TypeId::of::<Second>()].into_iter());
    assert_eq!(INITIALIZATIONS.load(Ordering::Relaxed), 0);

    let shared = locks.clone();
    let first = locks.get(&TypeInfo::of::<First>(), RegistrationId(0));
    let again = shared.get(&TypeInfo::of::<First>(), RegistrationId(0));
    assert!(matches!(first, LockRef::Indexed(_)));
    assert!(ptr::eq(&first.0, &again.0));
    assert_eq!(INITIALIZATIONS.load(Ordering::Relaxed), 1);

    let second = locks.get(&TypeInfo::of::<Second>(), RegistrationId(1));
    assert!(!ptr::eq(&first.0, &second.0));
    assert_eq!(INITIALIZATIONS.load(Ordering::Relaxed), 2);
}

#[test]
#[should_panic(expected = "invalid lock index 1 for type froodi::lock::tests::First")]
fn mismatched_index_panics() {
    let locks = TypeKeyedLocks::<usize>::with_indexed_keys(2, [TypeId::of::<First>(), TypeId::of::<Second>()].into_iter());
    locks.get(&TypeInfo::of::<First>(), RegistrationId(1));
}

#[test]
#[should_panic(expected = "invalid lock index 4294967295 for type froodi::lock::tests::First")]
fn absent_index_panics() {
    let locks = TypeKeyedLocks::<usize>::with_indexed_keys(1, [TypeId::of::<First>()].into_iter());
    locks.get(&TypeInfo::of::<First>(), RegistrationId::UNINDEXED_LOCK);
}

#[test]
fn map_locks_are_shared_across_clones() {
    let locks = TypeKeyedLocks::<usize>::default();
    let shared = locks.clone();
    let first = locks.get(&TypeInfo::of::<First>(), RegistrationId::UNINDEXED_LOCK);
    assert!(matches!(first, LockRef::Map(_)));
    assert!(ptr::eq(
        &*first,
        &*shared.get(&TypeInfo::of::<First>(), RegistrationId::UNINDEXED_LOCK)
    ));
}
