#[cfg(feature = "async")]
use crate::{
    async_impl::{Container as AsyncContainer, TypedContainer as AsyncTypedContainer, TypedContainerExt as _},
    compiled_async_registry as async_registry,
};
use crate::{
    compiled_registry as registry, instance,
    utils::thread_safety::RcThreadSafety,
    Config, Container, Context,
    DefaultScope::{App, Request, Runtime},
    Inject, InjectTransient, InstantiateErrorKind, TypedContainer, TypedContainerExt as _,
};
use alloc::string::String;
#[cfg(feature = "async")]
use alloc::string::ToString as _;
use core::sync::atomic::{AtomicUsize, Ordering};

struct Service(u32, u16);

fn service(Inject(app): Inject<u32>, InjectTransient(request): InjectTransient<u16>) -> Result<Service, InstantiateErrorKind> {
    Ok(Service(*app, request))
}

#[test]
fn typed_fragments_and_scope_builders_keep_provider_proofs() {
    let fragment = registry! { provide(Request, service) };
    let captured = String::from("captured");
    let container = TypedContainer::new(registry! {
        extend(fragment),
        provide(App, instance(7u32)),
        provide(Request, instance(11u16)),
        provide(App, move || Ok::<_, InstantiateErrorKind>(captured.clone())),
    });
    assert_eq!(&*container.get::<String>().unwrap(), "captured");
    assert!(container.get::<Service>().is_err());

    let child = container.clone().enter_build().unwrap();
    let value = child.get::<Service>().unwrap();
    assert_eq!((value.0, value.1), (7, 11));
    assert!(RcThreadSafety::ptr_eq(&value, &child.clone().get::<Service>().unwrap()));
    let transient = child.get_transient::<Service>().unwrap();
    assert_eq!((transient.0, transient.1), (7, 11));

    let mut context = Context::new();
    context.insert(19u16);
    context.insert(false);
    let child = container
        .clone()
        .enter()
        .with_scope(Request)
        .with_context(context.clone())
        .build()
        .unwrap();
    assert_eq!(*child.get::<u16>().unwrap(), 19);
    assert_eq!(child.get_transient::<u16>().unwrap(), 11);
    assert!(!*child.into_container().get::<bool>().unwrap());
    let child = container
        .clone()
        .enter()
        .with_context(context.clone())
        .with_scope(Request)
        .build()
        .unwrap();
    assert_eq!(*child.get::<u16>().unwrap(), 19);
    let child = container.clone().enter().with_context(context).build().unwrap();
    assert_eq!(*child.get::<u16>().unwrap(), 19);
    let child = container.enter().with_scope(Request).build().unwrap();
    assert_eq!(*child.get::<u16>().unwrap(), 11);
    assert!(child.into_container().get::<bool>().is_err());
}

#[test]
fn cache_policy_and_finalizers_use_the_original_container() {
    struct Value;
    struct Fresh;

    let calls = RcThreadSafety::new(AtomicUsize::new(0));
    let finalized = RcThreadSafety::new(AtomicUsize::new(0));
    let container = TypedContainer::new(registry! {
        provide(App, {
            let calls = calls.clone();
            move || { calls.fetch_add(1, Ordering::SeqCst); Ok::<_, InstantiateErrorKind>(Value) }
        }, finalizer = {
            let finalized = finalized.clone();
            move |_: RcThreadSafety<Value>| { finalized.fetch_add(1, Ordering::SeqCst); }
        }),
        provide(App, || Ok::<_, InstantiateErrorKind>(Fresh), config = Config { cache_provides: false }),
    });
    let first = container.get::<Value>().unwrap();
    let second = container.clone().get::<Value>().unwrap();
    assert!(RcThreadSafety::ptr_eq(&first, &second));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let _ = container.get_transient::<Value>().unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(!RcThreadSafety::ptr_eq(
        &container.get::<Fresh>().unwrap(),
        &container.get::<Fresh>().unwrap()
    ));
    container.close();
    assert_eq!(finalized.load(Ordering::SeqCst), 1);
}

#[test]
fn empty_registries_and_explicit_start_scopes_keep_implicit_providers() {
    let container = TypedContainer::new(registry!());
    assert!(container.get::<Container>().is_ok());
    assert!(container.get_transient::<Container>().is_ok());

    let container = TypedContainer::new_with_start_scope(registry! { provide(Runtime, instance(7u32)) }, Runtime);
    assert_eq!(*container.get::<u32>().unwrap(), 7);
    assert_eq!(*container.enter_build().unwrap().get::<u32>().unwrap(), 7);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_lookup_preserves_sync_fallback_scopes_and_finalizers() {
    let fragment = registry! { provide(App, instance(7u32)) };
    let finalized = RcThreadSafety::new(AtomicUsize::new(0));
    let container = AsyncTypedContainer::new(async_registry! {
        extend(fragment),
        provide(Request, async |value: Inject<u32>| Ok::<_, InstantiateErrorKind>(value.0.to_string()), finalizer = {
            let finalized = finalized.clone();
            move |_: RcThreadSafety<String>| {
                let finalized = finalized.clone();
                async move { finalized.fetch_add(1, Ordering::SeqCst); }
            }
        }),
    });
    assert_eq!(*container.get::<u32>().await.unwrap(), 7);
    assert_eq!(container.get_transient::<u32>().await.unwrap(), 7);
    assert!(container.get::<String>().await.is_err());

    let child = container.clone().enter_build().unwrap();
    let first = child.get::<String>().await.unwrap();
    assert!(RcThreadSafety::ptr_eq(&first, &child.clone().get::<String>().await.unwrap()));
    assert_eq!(child.get_transient::<String>().await.unwrap(), "7");
    child.close().await;
    assert_eq!(finalized.load(Ordering::SeqCst), 1);

    let mut context = Context::new();
    context.insert(String::from("context"));
    let child = container
        .clone()
        .enter()
        .with_scope(Request)
        .with_context(context.clone())
        .build()
        .unwrap();
    assert_eq!(&*child.get::<String>().await.unwrap(), "context");
    let child = container
        .clone()
        .enter()
        .with_context(context.clone())
        .with_scope(Request)
        .build()
        .unwrap();
    assert_eq!(&*child.get::<String>().await.unwrap(), "context");
    let child = container.clone().enter().with_context(context).build().unwrap();
    assert_eq!(&*child.get::<String>().await.unwrap(), "context");
    let child = container.enter().with_scope(Request).build().unwrap();
    assert_eq!(&*child.into_container().get::<String>().await.unwrap(), "7");

    let empty = AsyncTypedContainer::new_with_start_scope(async_registry!(), Runtime);
    assert!(empty.get::<Container>().await.is_ok());
    assert!(empty.get::<AsyncContainer>().await.is_ok());
}

#[cfg(feature = "thread_safe")]
#[test]
fn provider_proofs_add_no_clone_or_thread_safety_bounds() {
    fn send_sync<T: Send + Sync>(_: &T) {}

    struct Value;
    let container = TypedContainer::new(registry! { provide(App, || Ok::<_, InstantiateErrorKind>(Value)) });
    send_sync(&container);
    assert!(container.clone().get::<Value>().is_ok());
}
