#![cfg(feature = "async")]

use core::sync::atomic::{AtomicU8, Ordering};
use froodi::utils::thread_safety::RcThreadSafety;
use froodi::{
    async_impl::Container, instance, registry, Config, DefaultScope::*, Inject, InjectTransient, InstantiateErrorKind, ResolveErrorKind,
};

struct Value<const ID: usize>(usize);

mod factories {
    use super::*;

    pub fn synchronous(Inject(value): Inject<usize>) -> Result<Value<0>, InstantiateErrorKind> {
        Ok(Value(*value + 1))
    }

    pub async fn asynchronous(Inject(value): Inject<Value<0>>) -> Result<Value<1>, InstantiateErrorKind> {
        Ok(Value(value.0 + 1))
    }

    pub fn generic<const ID: usize>() -> Result<Value<ID>, InstantiateErrorKind> {
        Ok(Value(ID))
    }

    pub async fn generic_async<const ID: usize>() -> Result<Value<ID>, InstantiateErrorKind> {
        Ok(Value(ID))
    }
}

#[tokio::test]
async fn inferred_kinds_support_named_factories_aliases_closures_and_fragments() {
    use factories::asynchronous as imported_factory;

    let alias = imported_factory;
    let captured_sync = 10;
    let captured_async = 20;
    let infrastructure = registry! {
        provide(App, instance(7usize)),
        provide(App, factories::synchronous),
        provide(App, alias),
    };
    let app = Container::new(registry! {
        scope(App) [
            provide(factories::generic::<2>),
            provide(factories::generic_async::<3>),
            provide(|| Ok(Value::<4>(4))),
            provide(move || Ok(Value::<5>(captured_sync))),
            provide(|| async { Ok(Value::<6>(6)) }),
            provide(move || async move { Ok(Value::<7>(captured_async)) }),
            provide(|Inject(value): Inject<Value<0>>| Ok(Value::<8>(value.0))),
        ],
        scope(Request) [
            provide(|Inject(value): Inject<Value<1>>, InjectTransient(fresh): InjectTransient<Value<0>>| async move {
                Ok(Value::<9>(value.0 + fresh.0))
            }),
        ],
        extend(infrastructure),
    });
    assert_eq!(app.get::<Value<2>>().await.unwrap().0, 2);
    assert_eq!(app.get::<Value<3>>().await.unwrap().0, 3);
    assert_eq!(app.get::<Value<4>>().await.unwrap().0, 4);
    assert_eq!(app.get::<Value<5>>().await.unwrap().0, 10);
    assert_eq!(app.get::<Value<6>>().await.unwrap().0, 6);
    assert_eq!(app.get::<Value<7>>().await.unwrap().0, 20);
    assert_eq!(app.get::<Value<8>>().await.unwrap().0, 8);
    let request = app.clone().enter().with_scope(Request).build().unwrap();
    assert_eq!(request.get_transient::<Value<9>>().await.unwrap().0, 17);
    request.close().await;
    app.close().await;
}

#[tokio::test]
async fn mixed_factories_preserve_original_finalizers_after_erasure() {
    let sync_finishes = RcThreadSafety::new(AtomicU8::new(0));
    let async_finishes = RcThreadSafety::new(AtomicU8::new(0));
    let fragment = registry! {
        scope(App) [
            provide(factories::generic::<10>, finalizer = {
                let finishes = sync_finishes.clone();
                move |_: RcThreadSafety<Value<10>>| { finishes.fetch_add(1, Ordering::SeqCst); }
            }),
            provide(factories::generic_async::<13>, finalizer = {
                let finishes = async_finishes.clone();
                move |_: RcThreadSafety<Value<13>>| {
                    let finishes = finishes.clone();
                    async move { finishes.fetch_add(1, Ordering::SeqCst); }
                }
            }),
            provide(|Inject(value): Inject<Value<10>>| Ok(Value::<14>(value.0))),
        ],
    }
    .into_registry();
    let app = Container::new(registry! { extend(fragment) });
    for _ in 0..2 {
        app.get::<Value<10>>().await.unwrap();
        app.get::<Value<13>>().await.unwrap();
        assert_eq!(app.get::<Value<14>>().await.unwrap().0, 10);
        app.close().await;
    }
    app.close().await;
    assert_eq!(sync_finishes.load(Ordering::SeqCst), 2);
    assert_eq!(async_finishes.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn uncached_mixed_values_keep_finalizer_ownership_in_their_scope() {
    let finishes = RcThreadSafety::new(AtomicU8::new(0));
    let app = Container::new(registry! {
        provide(App, factories::generic::<15>, finalizer = {
            let finishes = finishes.clone();
            move |_: RcThreadSafety<Value<15>>| { finishes.fetch_add(1, Ordering::SeqCst); }
        }),
        provide(Request, factories::generic_async::<16>, config = Config { cache_provides: false }, finalizer = {
            let finishes = finishes.clone();
            move |_: RcThreadSafety<Value<16>>| {
                let finishes = finishes.clone();
                async move { finishes.fetch_add(1, Ordering::SeqCst); }
            }
        }),
    });
    let request = app.clone().enter().with_scope(Request).build().unwrap();
    request.get::<Value<15>>().await.unwrap();
    request.get::<Value<16>>().await.unwrap();
    request.get::<Value<16>>().await.unwrap();
    request.close().await;
    assert_eq!(finishes.load(Ordering::SeqCst), 2);
    app.close().await;
    assert_eq!(finishes.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn get_caches_same_instance_and_runs_instantiator_once() {
    struct Cached(u8);

    let call_count = RcThreadSafety::new(AtomicU8::new(0));

    let app_container = Container::new(registry! {
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

    let app_container = Container::new(registry! {
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

    let app_container = Container::new(registry! {
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

    let app_container = Container::new(registry! {
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

    let app_container = Container::new(registry! {
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

    let app_container = Container::new(registry! {
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
