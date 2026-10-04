extern crate std;

use crate::{
    async_impl::{Container, RegistryWithSync},
    declare, instance, registry,
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
#[should_panic(expected = "invalid async registry")]
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
