extern crate std;

use crate::{
    async_impl::Container,
    declare, instance, registry,
    DefaultScope::{App, Request},
    Inject, InstantiateErrorKind, Registry,
};

#[derive(Clone)]
struct Dependency(usize);
struct Service(usize);
struct Unrelated;

async fn service(Inject(dependency): Inject<Dependency>) -> Result<Service, InstantiateErrorKind> {
    Ok(Service(dependency.0))
}

async fn unrelated() -> Result<Unrelated, InstantiateErrorKind> {
    Ok(Unrelated)
}

fn native_service() -> Registry {
    native_async_registry! {
        provide(App, service),
        provide(App, declare::<Dependency>()),
    }
}

#[test]
#[should_panic(expected = "unreachable dependency")]
fn rejects_narrower_sync_scope_without_unrelated_async_executor() {
    let _ = Container::new(registry! {
        extend(native_service(), registry! {
            provide(Request, instance(Dependency(1))),
        }),
    });
}

#[test]
#[should_panic(expected = "unreachable dependency")]
fn rejects_narrower_sync_scope_with_unrelated_async_executor() {
    let _ = Container::new(registry! {
        provide(App, unrelated),
        extend(native_service(), registry! {
            provide(Request, instance(Dependency(1))),
        }),
    });
}

#[tokio::test]
async fn accepts_accessible_sync_scope_without_unrelated_async_executor() {
    let container = Container::new(registry! {
        extend(native_service(), registry! {
            provide(App, instance(Dependency(1))),
        }),
    });
    assert_eq!(container.get::<Service>().await.unwrap().0, 1);
}

#[tokio::test]
async fn accepts_accessible_sync_scope_with_unrelated_async_executor() {
    let container = Container::new(registry! {
        provide(App, unrelated),
        extend(native_service(), registry! {
            provide(App, instance(Dependency(1))),
        }),
    });
    assert_eq!(container.get::<Service>().await.unwrap().0, 1);
}

#[tokio::test]
async fn validation_uses_async_provider_before_narrower_sync_provider() {
    let container = Container::new(registry! {
        extend(
            native_async_registry! {
                provide(App, service),
                provide(App, async || Ok::<_, InstantiateErrorKind>(Dependency(2))),
            },
            registry! {
                provide(Request, instance(Dependency(1))),
            },
        ),
    });
    assert_eq!(container.get::<Service>().await.unwrap().0, 2);
}

#[test]
#[should_panic(expected = "invalid registry")]
fn validation_rejects_narrower_async_provider_even_when_sync_provider_is_accessible() {
    let _ = Container::new(registry! {
        extend(
            native_service(),
            native_async_registry! {
                provide(Request, async || Ok::<_, InstantiateErrorKind>(Dependency(2))),
            },
            registry! {
                provide(App, instance(Dependency(1))),
            },
        ),
    });
}

#[test]
#[should_panic(expected = "unreachable dependency")]
fn erased_cross_namespace_dependencies_are_validated_at_construction() {
    let _ = Container::new(native_async_registry! {
        provide(App, service),
        provide(App, declare::<Dependency>()),
        extend(native_registry! {
            provide(Request, instance(Dependency(1))),
        }),
    });
}

#[tokio::test]
async fn erased_same_type_providers_keep_execution_specific_selection() {
    fn sync_service(Inject(dependency): Inject<Dependency>) -> Result<usize, InstantiateErrorKind> {
        Ok(dependency.0)
    }

    for async_first in [false, true] {
        let sync = native_registry! {
            provide(App, instance(Dependency(1))),
            provide(App, sync_service),
        };
        let asynchronous = native_async_registry! {
            provide(App, async || Ok::<_, InstantiateErrorKind>(Dependency(2))),
            provide(App, service),
        };
        let container = if async_first {
            Container::new(registry! { extend(asynchronous, sync) })
        } else {
            Container::new(registry! { extend(sync, asynchronous) })
        };
        assert_eq!(container.get::<Dependency>().await.unwrap().0, 2);
        assert_eq!(container.get::<Service>().await.unwrap().0, 2);
        assert_eq!(*container.get::<usize>().await.unwrap(), 1);
        assert_eq!(container.sync.get::<Dependency>().unwrap().0, 1);
    }
}

#[test]
fn sync_container_accepts_unrelated_async_registrations_but_never_executes_them() {
    let container = crate::Container::new(registry! {
        provide(App, instance(Dependency(7))),
        provide(App, service),
    });
    assert_eq!(container.get::<Dependency>().unwrap().0, 7);
    assert!(matches!(
        container.get::<Service>(),
        Err(crate::ResolveErrorKind::AsyncRequired { .. })
    ));
    assert!(matches!(
        container.get_transient::<Service>(),
        Err(crate::ResolveErrorKind::AsyncRequired { .. })
    ));
}

#[test]
#[should_panic(expected = "Synchronous registration")]
fn erased_sync_edge_to_async_only_provider_is_rejected() {
    let consumer = native_registry! {
        provide(App, declare::<Dependency>()),
        provide(App, |Inject(dependency): Inject<Dependency>| Ok::<_, InstantiateErrorKind>(dependency.0)),
    };
    let dependency = registry! {
        provide(App, async || Ok::<_, InstantiateErrorKind>(Dependency(1))),
    }
    .into_registry();
    let _ = crate::Container::new(registry! { extend(consumer, dependency) });
}

#[tokio::test]
async fn both_execution_views_share_registry_storage_after_erasure() {
    let erased: Registry = registry! {
        provide(App, instance(Dependency(7))),
        provide(App, service),
    }
    .into_registry();
    let container = Container::new(erased.clone());
    assert!(crate::utils::thread_safety::RcThreadSafety::ptr_eq(
        &container.inner.registry,
        &container.sync.inner.registry
    ));
    assert_eq!(container.get::<Service>().await.unwrap().0, 7);
    assert_eq!(crate::Container::new(erased).get::<Dependency>().unwrap().0, 7);
}

#[tokio::test]
async fn erased_same_type_providers_retain_their_own_scope_and_finalizer() {
    use crate::utils::thread_safety::RcThreadSafety;
    use core::sync::atomic::{AtomicUsize, Ordering};

    let sync_finishes = RcThreadSafety::new(AtomicUsize::new(0));
    let async_finishes = RcThreadSafety::new(AtomicUsize::new(0));
    let sync = native_registry! {
        provide(App, instance(Dependency(1)), finalizer = {
            let finishes = sync_finishes.clone();
            move |_: RcThreadSafety<Dependency>| { finishes.fetch_add(1, Ordering::SeqCst); }
        }),
        provide(App, |dependency: crate::InjectTransient<Dependency>| Ok::<_, InstantiateErrorKind>(dependency.0.0)),
    };
    let asynchronous = native_async_registry! {
        provide(Request, async || Ok::<_, InstantiateErrorKind>(Dependency(2)), finalizer = {
            let finishes = async_finishes.clone();
            move |_: RcThreadSafety<Dependency>| {
                let finishes = finishes.clone();
                async move { finishes.fetch_add(1, Ordering::SeqCst); }
            }
        }),
        provide(Request, service),
    };
    let app = Container::new(registry! { extend(sync, asynchronous) });
    let request = app.clone().enter().with_scope(Request).build().unwrap();
    assert_eq!(*request.get::<usize>().await.unwrap(), 1);
    assert_eq!(request.get::<Service>().await.unwrap().0, 2);
    // A transient dependency never enters its finalizer queue.
    assert_eq!(request.sync.get::<Dependency>().unwrap().0, 1);
    request.close().await;
    assert_eq!(async_finishes.load(Ordering::SeqCst), 1);
    assert_eq!(sync_finishes.load(Ordering::SeqCst), 0);
    app.close().await;
    assert_eq!(sync_finishes.load(Ordering::SeqCst), 1);
}
