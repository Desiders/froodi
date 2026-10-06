use froodi::{instance, registry, Config, Container, DefaultScope::App};
use std::sync::atomic::{AtomicUsize, Ordering};

use froodi as scope_api;
#[path = "ui/registry/support/scopes.rs"]
mod scopes;

#[cfg(not(feature = "thread_safe"))]
use std::rc::Rc as Shared;
#[cfg(feature = "thread_safe")]
use std::sync::Arc as Shared;

#[derive(Clone)]
struct Repository(usize);

#[derive(froodi::Construct)]
struct Service {
    repository: Shared<Repository>,
}

#[derive(froodi::Construct)]
struct ExplicitService {
    #[di(inject)]
    repository: Shared<Repository>,
}

struct RequestId(usize);

#[derive(froodi::Construct)]
struct Mixed {
    cached: Shared<RequestId>,
    #[di(inject_transient)]
    first: RequestId,
    #[di(inject_transient)]
    second: RequestId,
}

#[derive(froodi::Construct)]
struct MixedTuple(#[di(inject)] Shared<Repository>, #[di(inject_transient)] RequestId);

#[derive(froodi::Construct)]
struct TransientGeneric<T> {
    #[di(inject_transient)]
    value: T,
}

#[derive(froodi::Construct)]
struct TransientPointer {
    #[di(inject_transient)]
    repository: Shared<Repository>,
}

#[derive(froodi::Construct)]
struct Pair(Shared<Service>, Shared<Repository>);

#[derive(froodi::Construct)]
struct Unit;

#[derive(froodi::Construct)]
struct Generic<T> {
    value: Shared<T>,
}

trait Greeting: Send + Sync {
    fn text(&self) -> &'static str;
}

struct Greeter;

impl Greeting for Greeter {
    fn text(&self) -> &'static str {
        "hello"
    }
}

#[derive(froodi::Construct)]
struct TraitService {
    greeter: Shared<dyn Greeting>,
}

#[derive(froodi::Construct)]
struct BoxedTraitService {
    greeter: Shared<Box<dyn Greeting>>,
    #[di(inject_transient)]
    fresh: Box<dyn Greeting>,
}

#[test]
fn constructed_types_use_normal_registry_linking_and_cache() {
    let container = Container::new(registry! {
        scope(App) [
            provide(instance(Repository(7))),
            provide::<Service>(),
            provide::<ExplicitService>(),
            provide::<Pair>(),
            provide::<Unit>(),
            provide::<Generic<Repository>>(),
        ]
    });
    let pair = container.get::<Pair>().unwrap();
    assert_eq!(pair.0.repository.0, 7);
    assert_eq!(pair.1 .0, 7);
    assert!(Shared::ptr_eq(&pair.0, &container.get::<Service>().unwrap()));
    container.get::<Unit>().unwrap();
    assert_eq!(container.get::<Generic<Repository>>().unwrap().value.0, 7);
    assert!(Shared::ptr_eq(&container.get::<ExplicitService>().unwrap().repository, &pair.1));
}

#[test]
fn mixed_fields_preserve_modes_and_repeated_dependency_order() {
    let calls = Shared::new(AtomicUsize::new(0));
    let container = Container::new(registry! {
        scope(App) [
            provide(instance(Repository(7))),
            provide({
                let calls = calls.clone();
                move || Ok::<_, froodi::InstantiateErrorKind>(RequestId(calls.fetch_add(1, Ordering::SeqCst) + 1))
            }),
            provide::<Mixed>(),
            provide::<MixedTuple>(),
            provide::<TransientGeneric<RequestId>>(),
        ],
    });
    let first = container.get::<Mixed>().unwrap();
    assert_eq!((first.cached.0, first.first.0, first.second.0), (1, 2, 3));
    assert!(Shared::ptr_eq(&first, &container.get::<Mixed>().unwrap()));
    assert_eq!(calls.load(Ordering::SeqCst), 3);

    let fresh = container.get_transient::<Mixed>().unwrap();
    assert!(Shared::ptr_eq(&first.cached, &fresh.cached));
    assert_eq!((fresh.first.0, fresh.second.0), (4, 5));
    let tuple = container.get::<MixedTuple>().unwrap();
    assert_eq!((tuple.0 .0, tuple.1 .0), (7, 6));
    assert_eq!(container.get::<TransientGeneric<RequestId>>().unwrap().value.0, 7);
}

#[test]
fn transient_mode_uses_the_exact_field_type_including_pointer_types() {
    let repository = Shared::new(Repository(7));
    let container = Container::new(registry! {
        scope(App) [
            provide(instance(repository.clone())),
            provide::<TransientPointer>(),
        ],
    });
    assert!(Shared::ptr_eq(
        &container.get::<TransientPointer>().unwrap().repository,
        &repository
    ));
}

#[test]
fn deriving_does_not_register_a_type() {
    let container = Container::new(registry! { provide(App, instance(Repository(7))) });
    assert!(container.get::<Service>().is_err());
}

#[test]
fn trait_object_fields_use_a_sized_shared_pointer_provider() {
    let container = Container::new(registry! {
        provide(App, || Ok::<_, froodi::InstantiateErrorKind>(Shared::new(Greeter) as Shared<dyn Greeting>)),
        scope(App) [ provide::<TraitService>(), ],
    });
    assert_eq!(container.get::<TraitService>().unwrap().greeter.text(), "hello");
}

#[test]
fn boxed_trait_fields_support_both_dependency_modes() {
    let container = Container::new(registry! {
        scope(App) [
            provide(|| Ok::<_, froodi::InstantiateErrorKind>(froodi::boxed!(Greeter; Greeting))),
            provide::<BoxedTraitService>(),
        ],
    });
    let service = container.get::<BoxedTraitService>().unwrap();
    assert_eq!(service.greeter.text(), "hello");
    assert_eq!(service.fresh.text(), "hello");
    assert!(Shared::ptr_eq(&service.greeter, &container.get::<Box<dyn Greeting>>().unwrap()));
}

#[test]
fn construct_resolves_in_custom_scopes_and_retains_parent_dependencies() {
    let container = Container::new(registry! {
        provide(scopes::App, instance(Repository(7))),
        scope(scopes::Request) [ provide::<ExplicitService>(), ],
    });
    assert!(container.get::<ExplicitService>().is_err());
    let request = container.clone().enter().with_scope(scopes::Request).build().unwrap();
    assert!(Shared::ptr_eq(
        &request.get::<ExplicitService>().unwrap().repository,
        &container.get::<Repository>().unwrap(),
    ));
}

#[test]
fn transient_field_resolution_errors_use_the_normal_error_path() {
    let container = Container::new(registry! {
        scope(App) [
            provide(|| Err::<RequestId, _>(froodi::InstantiateErrorKind::Custom(anyhow::anyhow!("request id unavailable")))),
            provide::<TransientGeneric<RequestId>>(),
        ],
    });
    let error = match container.get::<TransientGeneric<RequestId>>() {
        Ok(_) => panic!("a failed dependency unexpectedly constructed a service"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("request id unavailable"));
}

#[test]
fn construct_accepts_the_existing_registration_options() {
    let finalized = Shared::new(AtomicUsize::new(0));
    let container = Container::new(registry! {
        provide(App, instance(Repository(7))),
        scope(App) [
            provide::<Service>(config = Config { cache_provides: false }, finalizer = {
                let finalized = finalized.clone();
                move |_: Shared<Service>| { finalized.fetch_add(1, Ordering::SeqCst); }
            }),
        ],
    });
    assert!(!Shared::ptr_eq(
        &container.get::<Service>().unwrap(),
        &container.get::<Service>().unwrap()
    ));
    container.close();
    assert_eq!(finalized.load(Ordering::SeqCst), 2);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn synchronous_construct_works_in_a_mixed_async_registry() {
    let container = froodi::async_impl::Container::new(registry! {
        provide(App, instance(Repository(7))),
        scope(App) [ provide::<Service>(), ],
        scope(App) [ provide::<TransientGeneric<RequestId>>(), ],
        provide(App, || Ok::<_, froodi::InstantiateErrorKind>(RequestId(11))),
        provide(App, async |froodi::Inject(service): froodi::Inject<Service>| Ok::<_, froodi::InstantiateErrorKind>(service.repository.0)),
    });
    assert_eq!(container.get::<Service>().await.unwrap().repository.0, 7);
    assert_eq!(container.get::<TransientGeneric<RequestId>>().await.unwrap().value.0, 11);
    assert_eq!(*container.get::<usize>().await.unwrap(), 7);
}
