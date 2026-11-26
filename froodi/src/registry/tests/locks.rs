#[cfg(feature = "async")]
use crate::async_impl::Container as AsyncContainer;
use crate::{
    lock::PerTypeSyncLocks,
    registry,
    registry::{frontend::prepare, RegistrationId},
    Container,
    DefaultScope::App,
    Registry, TypeInfo,
};
#[cfg(feature = "async")]
use alloc::vec::Vec;
use core::ptr;

#[test]
fn indexed_and_type_lookups_share_a_lock() {
    let registry = registry! { provide(App, || Ok(7_u32)) }.into_registry();
    let type_info = TypeInfo::of::<u32>();
    let keys = [
        type_info.clone(),
        type_info.clone(),
        TypeInfo::of::<Container>(),
        #[cfg(feature = "async")]
        TypeInfo::of::<AsyncContainer>(),
    ];
    let registry = prepare(registry, false, Some(keys.into()));
    let by_type = registry.get(&type_info).unwrap().metadata.lock_id;
    let first_id = registry.indexed[0].1.sync().unwrap().metadata.lock_id;
    let duplicate_id = registry.indexed[1].1.sync().unwrap().metadata.lock_id;
    let (_, mut indexed_keys) = registry.indexed_lock_keys(false).unwrap();
    let expected = indexed_keys.position(|id| id == type_info.id).unwrap();

    assert_eq!(by_type.index(), expected);
    assert_eq!(first_id.index(), expected);
    assert_eq!(duplicate_id.index(), expected);

    let locks = PerTypeSyncLocks::from_registry(&registry);
    let ordinary = locks.get(&type_info, by_type);
    let linked = locks.get(&type_info, duplicate_id);
    assert!(ptr::eq(&*ordinary, &*linked));
}

#[test]
fn child_and_unindexed_containers_share_their_locks() {
    let app = Container::new(registry! { provide(App, || Ok(7_u32)) });
    let request = app.clone().enter_build().unwrap();
    let type_info = TypeInfo::of::<u32>();
    let id = app.inner.registry.get(&type_info).unwrap().metadata.lock_id;
    let parent = app.per_type_locks.get(&type_info, id);
    let child = request.per_type_locks.get(&type_info, id);
    assert!(ptr::eq(&*parent, &*child));

    let unindexed = Container::new(Registry::new_with_default_entries());
    assert!(unindexed.inner.registry.indexed.is_empty());
    assert!(unindexed.inner.registry.indexed_lock_keys(false).is_none());
    let implicit = unindexed.get::<Container>().unwrap();
    let container_type = TypeInfo::of::<Container>();
    let id = RegistrationId::UNINDEXED_LOCK;
    let outer = unindexed.per_type_locks.get(&container_type, id);
    let inner = implicit.per_type_locks.get(&container_type, id);
    assert!(ptr::eq(&*outer, &*inner));
}

#[cfg(feature = "async")]
#[test]
fn mixed_registry_indexes_sync_and_async_locks_separately() {
    let synchronous = registry! {
        provide(App, || Ok(7_u32)),
        provide(App, || Ok(11_u64)),
    }
    .into_registry();
    let asynchronous = registry! {
        provide(App, async || Ok(9_u32)),
        provide(App, async || Ok(true)),
    }
    .into_registry();
    let registry = prepare(registry! { extend(synchronous, asynchronous) }.into_registry(), false, None);
    let sync_id = registry.get(&TypeInfo::of::<u32>()).unwrap().metadata.lock_id.index();
    let async_id = registry.get_async(&TypeInfo::of::<u32>()).unwrap().metadata.lock_id.index();
    let (sync_count, sync_keys) = registry.indexed_lock_keys(false).unwrap();
    let (async_count, async_keys) = registry.indexed_lock_keys(true).unwrap();
    let sync_keys: Vec<_> = sync_keys.collect();
    let async_keys: Vec<_> = async_keys.collect();
    assert_eq!(sync_count, sync_keys.len());
    assert_eq!(async_count, async_keys.len());
    assert_eq!(sync_keys[sync_id], TypeInfo::of::<u32>().id);
    assert_eq!(async_keys[async_id], TypeInfo::of::<u32>().id);
    assert!(sync_keys.contains(&TypeInfo::of::<u64>().id));
    assert!(async_keys.contains(&TypeInfo::of::<bool>().id));
    assert!(!sync_keys.contains(&TypeInfo::of::<bool>().id));
    assert!(!async_keys.contains(&TypeInfo::of::<u64>().id));
    let indexed = registry.indexed.iter().find(|(key, _)| key.id == TypeInfo::of::<u32>().id).unwrap();
    assert_eq!(indexed.1.sync().unwrap().metadata.lock_id.index(), sync_id);
}
