#![cfg(feature = "async")]

use core::sync::atomic::{AtomicU8, Ordering};
use froodi::utils::thread_safety::RcThreadSafety;
use froodi::{async_impl::Container, async_registry, DefaultScope::*, Inject, InstantiateErrorKind, ResolveErrorKind};

#[tokio::test]
async fn get_caches_same_instance_and_runs_instantiator_once() {
    struct Cached(u8);

    let call_count = RcThreadSafety::new(AtomicU8::new(0));

    let app_container = Container::new(async_registry! {
        scope(App) [
            provide({
                let call_count = call_count.clone();
                move || {
                    let call_count = call_count.clone();
                    async move {
                        let n = call_count.fetch_add(1, Ordering::SeqCst);
                        Ok::<_, InstantiateErrorKind>(Cached(n))
                    }
                }
            }),
        ],
    });

    let first = app_container.get::<Cached>().await.unwrap();
    let second = app_container.get::<Cached>().await.unwrap();

    assert!(RcThreadSafety::ptr_eq(&first, &second));
    // fetch_add returned 0 -> single instantiation
    assert_eq!(first.0, 0);
    assert_eq!(second.0, 0);
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn get_transient_is_fresh_each_call() {
    struct Transient(u8);

    let call_count = RcThreadSafety::new(AtomicU8::new(0));

    let app_container = Container::new(async_registry! {
        scope(App) [
            provide({
                let call_count = call_count.clone();
                move || {
                    let call_count = call_count.clone();
                    async move {
                        let n = call_count.fetch_add(1, Ordering::SeqCst);
                        Ok::<_, InstantiateErrorKind>(Transient(n))
                    }
                }
            }),
        ],
    });

    let first = app_container.get_transient::<Transient>().await.unwrap();
    let second = app_container.get_transient::<Transient>().await.unwrap();
    let third = app_container.get_transient::<Transient>().await.unwrap();

    assert_eq!(first.0, 0);
    assert_eq!(second.0, 1);
    assert_eq!(third.0, 2);
    assert_eq!(call_count.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn inject_across_scopes_shares_leaf() {
    struct Leaf(u8);
    struct Mid(RcThreadSafety<Leaf>);

    let leaf_count = RcThreadSafety::new(AtomicU8::new(0));

    let app_container = Container::new(async_registry! {
        scope(App) [
            provide({
                let leaf_count = leaf_count.clone();
                move || {
                    let leaf_count = leaf_count.clone();
                    async move {
                        let n = leaf_count.fetch_add(1, Ordering::SeqCst);
                        Ok::<_, InstantiateErrorKind>(Leaf(n))
                    }
                }
            }),
        ],
        scope(Request) [
            provide(async |Inject(leaf): Inject<Leaf>| Ok::<_, InstantiateErrorKind>(Mid(leaf))),
        ],
    });

    let request_container = app_container.clone().enter().with_scope(Request).build().unwrap();

    let mid = request_container.get::<Mid>().await.unwrap();
    let leaf_direct = request_container.get::<Leaf>().await.unwrap();

    // Leaf injected into Mid is the same cached instance as the direct get
    assert!(RcThreadSafety::ptr_eq(&mid.0, &leaf_direct));
    assert_eq!(mid.0 .0, 0);
    assert_eq!(leaf_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn get_unregistered_returns_no_instantiator() {
    struct Registered;
    #[derive(Debug)]
    struct Unregistered;

    let app_container = Container::new(async_registry! {
        scope(App) [
            provide(async || Ok::<_, InstantiateErrorKind>(Registered)),
        ],
    });

    let err = app_container.get::<Unregistered>().await.unwrap_err();
    assert!(matches!(err, ResolveErrorKind::NoInstantiator { .. }));
}

#[tokio::test]
async fn get_request_scoped_from_app_returns_no_accessible() {
    struct AppDep;
    #[derive(Debug)]
    struct RequestDep;

    let app_container = Container::new(async_registry! {
        scope(App) [
            provide(async || Ok::<_, InstantiateErrorKind>(AppDep)),
        ],
        scope(Request) [
            provide(async || Ok::<_, InstantiateErrorKind>(RequestDep)),
        ],
    });

    let _app_dep = app_container.get::<AppDep>().await.unwrap();

    let err = app_container.get::<RequestDep>().await.unwrap_err();
    assert!(matches!(
        err,
        ResolveErrorKind::NoAccessible {
            expected_scope_data: _,
            actual_scope_data: _,
        }
    ));
}

#[tokio::test]
async fn close_runs_finalizer_and_resets_cache() {
    struct Closable(u8);

    let inst_count = RcThreadSafety::new(AtomicU8::new(0));
    let fin_count = RcThreadSafety::new(AtomicU8::new(0));

    let app_container = Container::new(async_registry! {
        scope(App) [
            provide(
                {
                    let inst_count = inst_count.clone();
                    move || {
                        let inst_count = inst_count.clone();
                        async move {
                            let n = inst_count.fetch_add(1, Ordering::SeqCst);
                            Ok::<_, InstantiateErrorKind>(Closable(n))
                        }
                    }
                },
                finalizer = {
                    let fin_count = fin_count.clone();
                    move |_: RcThreadSafety<Closable>| {
                        let fin_count = fin_count.clone();
                        async move {
                            fin_count.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                },
            ),
        ],
    });

    let first = app_container.get::<Closable>().await.unwrap();
    assert_eq!(first.0, 0);
    assert_eq!(inst_count.load(Ordering::SeqCst), 1);
    assert_eq!(fin_count.load(Ordering::SeqCst), 0);

    let cached = app_container.get::<Closable>().await.unwrap();
    assert!(RcThreadSafety::ptr_eq(&first, &cached));
    assert_eq!(inst_count.load(Ordering::SeqCst), 1);

    app_container.close().await;
    assert_eq!(fin_count.load(Ordering::SeqCst), 1);

    // close reset the cache -> fresh, non-ptr_eq instance
    let after = app_container.get::<Closable>().await.unwrap();
    assert!(!RcThreadSafety::ptr_eq(&first, &after));
    assert_eq!(after.0, 1);
    assert_eq!(inst_count.load(Ordering::SeqCst), 2);
}
