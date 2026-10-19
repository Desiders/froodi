extern crate std;

use alloc::{borrow::ToOwned, string::String, vec::Vec};
use core::{
    mem::size_of_val,
    sync::atomic::{AtomicUsize, Ordering},
};
use std::sync::Mutex;

#[cfg(feature = "async")]
use crate::async_impl::{instantiator::RegistrationInstantiator as AsyncRegistrationInstantiator, Container as AsyncContainer};
use crate::{
    fragment, instance,
    instantiator::RegistrationInstantiator,
    registry,
    registry::{RegistrationMetadata, Selected},
    utils::thread_safety::RcThreadSafety,
    Config as RegistrationConfig, Container,
    DefaultScope::App,
    Inject, InstantiateErrorKind, Registry,
};

#[derive(Clone)]
struct Config(u32);

struct Repository(u32);

fn make_repository(Inject(config): Inject<Config>) -> Result<Repository, InstantiateErrorKind> {
    Ok(Repository(config.0 + 1))
}

#[fragment(infrastructure(config))]
registry! {
    provide(App, instance(config)),
    provide(App, make_repository),
}

use infrastructure as infra_fragment;

struct Handler(u32);

impl Handler {
    fn new(repository: RcThreadSafety<Repository>) -> Self {
        Self(repository.0)
    }
}

#[fragment(application)]
registry! {
    provide(App, |Inject(repository)| Ok::<_, InstantiateErrorKind>(Handler::new(repository))),
}

#[test]
fn dependencies_link_across_fragments_in_both_directions() {
    let container = Container::new(registry! {
        extend_fragment(application!()),
        extend_fragment(infra_fragment!(Config(41))),
    });
    assert_eq!(container.get::<Handler>().unwrap().0, 42);
}

#[fragment(storage(settings))]
registry! {
    provide(App, instance(settings)),
}

#[fragment(borrowed(settings))]
registry! {
    provide(App, instance(settings.to_owned())),
}

#[test]
fn arguments_infer_types_and_borrow_without_template_generics() {
    let text = String::from("runtime");
    let container = Container::new(registry! {
        extend_fragment(storage!(7u32), storage!([0u8; 3]), borrowed!(text.as_str())),
    });
    assert_eq!(*container.get::<u32>().unwrap(), 7);
    assert_eq!(&**container.get::<String>().unwrap(), "runtime");
    assert_eq!(*container.get::<[u8; 3]>().unwrap(), [0; 3]);
}

#[cfg(feature = "async")]
#[derive(crate::Construct)]
struct Service {
    repository: RcThreadSafety<Repository>,
    #[di(inject_transient)]
    number: u64,
}

#[cfg(feature = "async")]
struct Database(u32);

#[cfg(feature = "async")]
struct AsyncHandler(u32);

#[cfg(feature = "async")]
async fn connect_database(Inject(config): Inject<Config>) -> Result<Database, InstantiateErrorKind> {
    Ok(Database(config.0))
}

#[cfg(feature = "async")]
async fn make_handler(
    Inject(database): Inject<Database>,
    Inject(repository): Inject<Repository>,
) -> Result<AsyncHandler, InstantiateErrorKind> {
    Ok(AsyncHandler(database.0 + repository.0))
}

#[cfg(feature = "async")]
#[fragment(mixed)]
registry! {
    scope(App) [
        provide(connect_database),
        provide(|| Ok::<_, InstantiateErrorKind>(7u64)),
        provide::<Service>(),
    ],
    provide(App, make_handler),
}

#[cfg(feature = "async")]
#[tokio::test(flavor = "current_thread")]
async fn mixed_construct_transient_and_both_dependency_directions() {
    let container = AsyncContainer::new(registry! {
        extend_fragment(mixed!()),
        extend_fragment(infra_fragment!(Config(41))),
    });
    assert_eq!(container.get::<AsyncHandler>().await.unwrap().0, 83);
    let service = container.get::<Service>().await.unwrap();
    assert_eq!(service.repository.0, 42);
    assert_eq!(service.number, 7);
}

struct Client(String);

#[fragment(clients(url))]
registry! {
    provide(App, move || Ok::<_, InstantiateErrorKind>(Client(url.clone()))),
}

#[test]
fn captures_moves_and_repeated_expansion_hygiene() {
    let container = Container::new(registry! {
        extend_fragment(clients!(String::from("https://example.invalid"))),
        extend_fragment(storage!(7u32)),
        extend_fragment(storage!(8u64)),
    });
    assert_eq!(&container.get::<Client>().unwrap().0, "https://example.invalid");
    assert_eq!(*container.get::<u32>().unwrap(), 7);
    assert_eq!(*container.get::<u64>().unwrap(), 8);
}

type Events = RcThreadSafety<Mutex<Vec<&'static str>>>;

fn record<Value>(events: &Events, name: &'static str, value: Value) -> Value {
    events.lock().unwrap().push(name);
    value
}

#[fragment(ordered(events, value))]
registry! {
    provide(
        record(&events, "scope", App),
        record(&events, "instantiator", instance(value)),
        config = record(&events, "config", RegistrationConfig::default()),
        finalizer = record(&events, "finalizer", |_: RcThreadSafety<u16>| {}),
    ),
}

#[test]
fn arguments_and_registration_expressions_keep_source_order() {
    let events = RcThreadSafety::new(Mutex::new(Vec::new()));
    let container = Container::new(registry! {
        provide(record(&events, "before", App), instance(true)),
        extend_fragment(ordered!(record(&events, "argument 1", events.clone()), record(&events, "argument 2", 7u16))),
        provide(record(&events, "after", App), instance(9u8)),
    });
    assert_eq!(
        *events.lock().unwrap(),
        [
            "before",
            "argument 1",
            "argument 2",
            "scope",
            "instantiator",
            "config",
            "finalizer",
            "after"
        ]
    );
    assert_eq!(*container.get::<u16>().unwrap(), 7);
}

#[fragment(outer(config))]
registry! {
    extend_fragment(infra_fragment!(config)),
}

#[test]
fn nested_fragments_and_explicit_erasure() {
    let native: Registry = registry! { extend_fragment(outer!(Config(41))) }.into_registry();
    let container = Container::new(registry! { extend(native) });
    assert_eq!(container.get::<Repository>().unwrap().0, 42);
}

struct Counted {
    clones: RcThreadSafety<AtomicUsize>,
}

impl Clone for Counted {
    fn clone(&self) -> Self {
        self.clones.fetch_add(1, Ordering::SeqCst);
        Self {
            clones: self.clones.clone(),
        }
    }
}

impl Counted {
    fn client(&self) -> Client {
        Client("captured".to_owned())
    }
}

#[fragment(counted(value))]
registry! {
    provide(App, move || Ok::<_, InstantiateErrorKind>(value.client())),
}

#[test]
fn fragment_binding_does_not_add_instantiator_clones() {
    for fragmented in [false, true] {
        let clones = RcThreadSafety::new(AtomicUsize::new(0));
        let value = Counted { clones: clones.clone() };
        let container = if fragmented {
            Container::new(registry! { extend_fragment(counted!(value)) })
        } else {
            Container::new(registry! { provide(App, move || Ok::<_, InstantiateErrorKind>(value.client())) })
        };
        assert_eq!(clones.load(Ordering::SeqCst), 0);
        container.get_transient::<Client>().unwrap();
        assert_eq!(clones.load(Ordering::SeqCst), 1);
        container.get_transient::<Client>().unwrap();
        assert_eq!(clones.load(Ordering::SeqCst), 2);
    }
}

#[cfg(feature = "async")]
struct RemoteClient(String);

#[cfg(feature = "async")]
#[fragment(captured_async(url))]
registry! {
    provide(App, move || {
        let url = url.clone();
        async move { Ok::<_, InstantiateErrorKind>(RemoteClient(url)) }
    }),
}

#[cfg(feature = "async")]
#[tokio::test(flavor = "current_thread")]
async fn captured_async_provider_and_named_alias() {
    let alias = connect_database;
    let container = AsyncContainer::new(registry! {
        extend_fragment(captured_async!("https://example.invalid".to_owned())),
        extend_fragment(infra_fragment!(Config(41))),
        provide(App, alias),
    });
    assert_eq!(&container.get::<RemoteClient>().await.unwrap().0, "https://example.invalid");
    assert_eq!(container.get::<Database>().await.unwrap().0, 41);
}

fn generic_factory<Value: Default>() -> Result<Value, InstantiateErrorKind> {
    Ok(Value::default())
}

#[fragment(generic_provider)]
registry! {
    provide(App, generic_factory::<u16>),
}

#[test]
fn generic_named_provider() {
    let container = Container::new(registry! { extend_fragment(generic_provider!()) });
    assert_eq!(*container.get::<u16>().unwrap(), 0);
}

#[test]
fn arguments_drop_at_the_fragment_boundary_and_moves_do_not_clone() {
    struct Guard(Events, &'static str);

    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.lock().unwrap().push(self.1);
        }
    }

    let events = RcThreadSafety::new(Mutex::new(Vec::new()));
    let container = Container::new(registry! {
        extend_fragment(discard!(Guard(events.clone(), "drop first"), Guard(events.clone(), "drop second"), 6u16)),
        provide(record(&events, "after", App), instance(false)),
    });
    assert_eq!(*events.lock().unwrap(), ["drop second", "drop first", "after"]);
    assert_eq!(*container.get::<u16>().unwrap(), 7);
}

fn assert_same_metadata(inline: &RegistrationMetadata, fragmented: &RegistrationMetadata) {
    assert_eq!(inline.scope_data, fragmented.scope_data);
    assert_eq!(inline.config.cache_provides, fragmented.config.cache_provides);
    assert_eq!(inline.dependencies, fragmented.dependencies);
}

#[test]
fn borrowed_mutable_arguments_are_consumed_without_escaping() {
    let mut value = 6u32;
    let container = Container::new(registry! { extend_fragment(increment!(&mut value)), });
    assert_eq!(value, 7);
    assert_eq!(*container.get::<u32>().unwrap(), 7);
}

#[test]
fn repeated_argument_use_does_not_repeat_the_argument_expression() {
    let calls = AtomicUsize::new(0);
    let container = Container::new(registry! {
        extend_fragment(repeated!({ calls.fetch_add(1, Ordering::SeqCst); 7u32 })),
    });
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(*container.get::<u32>().unwrap(), 7);
    assert_eq!(*container.get::<u64>().unwrap(), 7);
}

fn assert_same_registry(inline: &Registry, fragmented: &Registry) {
    assert_eq!(inline.scopes_data, fragmented.scopes_data);
    assert_eq!(inline.indexed.len(), fragmented.indexed.len());
    for ((inline_key, inline_entry), (fragment_key, fragment_entry)) in inline.indexed.iter().zip(&fragmented.indexed) {
        assert_eq!(inline_key, fragment_key);
        assert_eq!(inline_entry.linked_keys(), fragment_entry.linked_keys());
        match (&inline_entry.selected, &fragment_entry.selected) {
            (Selected::Sync(inline_data), Selected::Sync(fragment_data)) => {
                assert_same_metadata(&inline_data.metadata, &fragment_data.metadata);
                assert_eq!(inline_data.finalizer.is_some(), fragment_data.finalizer.is_some());
                match (&inline_data.instantiator, &fragment_data.instantiator) {
                    (RegistrationInstantiator::Linked(inline_executor), RegistrationInstantiator::Linked(fragment_executor)) => {
                        assert_eq!(
                            inline_executor.edges.iter().map(|id| id.index()).collect::<Vec<_>>(),
                            fragment_executor.edges.iter().map(|id| id.index()).collect::<Vec<_>>()
                        );
                        assert_eq!(size_of_val(inline_executor), size_of_val(fragment_executor));
                    }
                    _ => assert_eq!(inline_entry.is_linked(), fragment_entry.is_linked()),
                }
            }
            #[cfg(feature = "async")]
            (Selected::Async(inline_data), Selected::Async(fragment_data)) => {
                assert_same_metadata(&inline_data.metadata, &fragment_data.metadata);
                assert_eq!(inline_data.finalizer.is_some(), fragment_data.finalizer.is_some());
                match (&inline_data.instantiator, &fragment_data.instantiator) {
                    (AsyncRegistrationInstantiator::Linked(inline_executor), AsyncRegistrationInstantiator::Linked(fragment_executor)) => {
                        assert_eq!(
                            inline_executor.edges.iter().map(|id| id.index()).collect::<Vec<_>>(),
                            fragment_executor.edges.iter().map(|id| id.index()).collect::<Vec<_>>()
                        );
                        assert_eq!(size_of_val(inline_executor), size_of_val(fragment_executor));
                    }
                    _ => assert_eq!(inline_entry.is_linked(), fragment_entry.is_linked()),
                }
            }
            _ => panic!("fragment changed registration execution kind"),
        }
    }
}

#[test]
fn inline_and_fragmented_materialization_have_the_same_indexed_graph() {
    let inline = registry! {
        provide(App, instance(Config(41))),
        provide(App, make_repository),
        provide(App, |Inject(repository)| Ok::<_, InstantiateErrorKind>(Handler::new(repository))),
    }
    .into_registry();
    let fragmented = registry! {
        extend_fragment(infra_fragment!(Config(41)), application!()),
    }
    .into_registry();
    assert_same_registry(&inline, &fragmented);
}

#[test]
fn value_producing_macros_remain_valid_extensions() {
    macro_rules! values {
        () => {
            registry! { provide(App, instance(7u32)), }
        };
    }
    let container = Container::new(registry! { extend(values!()), });
    assert_eq!(*container.get::<u32>().unwrap(), 7);
}

#[test]
fn caller_arguments_keep_their_names_before_fragment_imports() {
    mod provider {
        pub struct Value;

        pub fn make() -> u32 {
            7
        }
    }

    struct Value;

    impl Value {
        fn make() -> u32 {
            11
        }
    }

    let container = Container::new(registry! { extend_fragment(imports!(Value::make())), });
    assert_eq!(*container.get::<u32>().unwrap(), 18);
    container.get::<provider::Value>().unwrap();

    let inline = Container::new(registry! {
        use provider::{make, Value};
        provide(App, instance(11 + make())),
        provide(App, || Ok::<_, InstantiateErrorKind>(Value)),
    });
    assert_eq!(*inline.get::<u32>().unwrap(), 18);
    inline.get::<provider::Value>().unwrap();
}

#[cfg(feature = "async")]
#[tokio::test(flavor = "current_thread")]
async fn mixed_finalizers_have_the_same_registration_data_and_lifecycle() {
    fn finish_sync(_: RcThreadSafety<u32>) {}

    async fn number() -> Result<u16, InstantiateErrorKind> {
        Ok(11)
    }

    async fn finish_async(_: RcThreadSafety<u16>) {}

    let fragmented = registry! { extend_fragment(finalizers!()), }.into_registry();
    let inline = registry! {
        provide(App, instance(7u32), finalizer = finish_sync, config = RegistrationConfig { cache_provides: false }),
        provide(App, number, finalizer = finish_async),
        provide(App, |value: Inject<u32>, other: Inject<u16>| async move { Ok::<_, InstantiateErrorKind>(u64::from(*value.0) + u64::from(*other.0)) }),
    }.into_registry();
    assert_same_registry(&inline, &fragmented);
    for registry in [inline, fragmented] {
        let container = AsyncContainer::new(registry);
        assert_eq!(*container.get::<u64>().await.unwrap(), 18);
        container.close().await;
    }
}

#[test]
fn a_non_clone_argument_moves_into_shared_provider_state() {
    struct Payload {
        value: String,
        drops: RcThreadSafety<AtomicUsize>,
    }

    impl Drop for Payload {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct PayloadView {
        payload: RcThreadSafety<Payload>,
    }

    let drops = RcThreadSafety::new(AtomicUsize::new(0));
    let container = Container::new(registry! {
        extend_fragment(shared_payload!(Payload { value: String::from("owned"), drops: drops.clone() }, String::from("shared"))),
    });
    assert_eq!(container.get_transient::<PayloadView>().unwrap().payload.value, "owned");
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(container);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[fragment(discard(_first_guard, _second_guard, value))]
registry! {
    provide(App, instance({
        let mut value = value;
        value += 1;
        value
    })),
}

#[fragment(increment(value))]
registry! {
    provide(App, instance({ *value += 1; *value })),
}

#[fragment(repeated(value))]
registry! {
    provide(App, instance(value)),
    provide(App, instance(u64::from(value))),
}

#[fragment(imports(value))]
registry! {
    use provider::{make, Value};
    provide(App, instance(value + make())),
    provide(App, || Ok::<_, InstantiateErrorKind>(Value)),
}

#[fragment(finalizers)]
registry! {
    provide(App, instance(7u32), finalizer = finish_sync, config = RegistrationConfig { cache_provides: false }),
    provide(App, number, finalizer = finish_async),
    provide(App, |value: Inject<u32>, other: Inject<u16>| async move { Ok::<_, InstantiateErrorKind>(u64::from(*value.0) + u64::from(*other.0)) }),
}

#[fragment(shared_payload(payload, prefix))]
registry! {
    provide(App, {
        let payload = RcThreadSafety::new(payload);
        move || {
            assert_eq!(prefix, "shared");
            Ok::<_, InstantiateErrorKind>(PayloadView { payload: payload.clone() })
        }
    }),
}
