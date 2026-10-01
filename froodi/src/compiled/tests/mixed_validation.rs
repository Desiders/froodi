extern crate std;

use crate::{
    async_impl::{Container, RegistryWithSync},
    async_registry as dynamic_async_registry, compiled_async_registry as async_registry, compiled_registry as registry, instance,
    registry as dynamic_registry,
    DefaultScope::{App, Request},
    Inject, InstantiateErrorKind,
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

fn native_service() -> RegistryWithSync {
    dynamic_async_registry! {
        provide(App, service),
    }
}

#[test]
#[should_panic(expected = "unreachable dependency")]
fn rejects_narrower_sync_scope_without_compiled_async_executor() {
    let _ = Container::new(async_registry! {
        extend(native_service(), registry! {
            provide(Request, instance(Dependency(1))),
        }),
    });
}

#[test]
#[should_panic(expected = "unreachable dependency")]
fn rejects_narrower_sync_scope_with_unrelated_compiled_async_executor() {
    let _ = Container::new(async_registry! {
        provide(App, unrelated),
        extend(native_service(), registry! {
            provide(Request, instance(Dependency(1))),
        }),
    });
}

#[tokio::test]
async fn accepts_accessible_sync_scope_without_compiled_async_executor() {
    let container = Container::new(async_registry! {
        extend(native_service(), registry! {
            provide(App, instance(Dependency(1))),
        }),
    });
    assert_eq!(container.get::<Service>().await.unwrap().0, 1);
}

#[tokio::test]
async fn accepts_accessible_sync_scope_with_unrelated_compiled_async_executor() {
    let container = Container::new(async_registry! {
        provide(App, unrelated),
        extend(native_service(), registry! {
            provide(App, instance(Dependency(1))),
        }),
    });
    assert_eq!(container.get::<Service>().await.unwrap().0, 1);
}

#[tokio::test]
async fn validation_uses_async_provider_before_narrower_sync_provider() {
    let container = Container::new(async_registry! {
        extend(
            dynamic_async_registry! {
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
#[should_panic(expected = "invalid compiled async registry")]
fn validation_rejects_narrower_async_provider_even_when_sync_provider_is_accessible() {
    let _ = Container::new(async_registry! {
        extend(
            native_service(),
            dynamic_async_registry! {
                provide(Request, async || Ok::<_, InstantiateErrorKind>(Dependency(2))),
            },
            registry! {
                provide(App, instance(Dependency(1))),
            },
        ),
    });
}

#[tokio::test]
async fn dynamic_only_cross_namespace_validation_stays_unchanged() {
    let container = Container::new(dynamic_async_registry! {
        provide(App, service),
        extend(dynamic_registry! {
            provide(Request, instance(Dependency(1))),
        }),
    });
    assert!(container.get::<Service>().await.is_err());
}
