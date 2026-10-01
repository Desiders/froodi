extern crate std;

#[cfg(feature = "async")]
use alloc::format;
use alloc::{
    collections::BTreeSet,
    string::{String, ToString as _},
    vec::Vec,
};

#[cfg(feature = "async")]
use crate::{
    async_impl::{Container as AsyncContainer, Instantiator as AsyncInstantiator},
    async_registry as dynamic_async_registry, compiled_async_registry as async_registry,
};
use crate::{
    compiled::IntoRegistry,
    compiled_registry as registry, context,
    errors::InstantiatorErrorKind,
    instance, registry as dynamic_registry, runtime,
    utils::thread_safety::RcThreadSafety,
    Config, Container as SyncContainer, Context,
    DefaultScope::{App, Request},
    Dependency, DependencyResolver, Inject, InjectTransient, InstantiateErrorKind, Instantiator, ResolveErrorKind, RuntimeDependency,
    TypeInfo,
};
use std::sync::Mutex;

struct Opaque;

impl DependencyResolver for Opaque {
    type Error = ResolveErrorKind;

    fn resolve(_: &SyncContainer) -> Result<Self, Self::Error> {
        Ok(Self)
    }

    #[cfg(feature = "async")]
    async fn resolve_async(_: &AsyncContainer) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

struct FailingResolver;
struct CustomResolverError;

impl From<CustomResolverError> for ResolveErrorKind {
    fn from(_: CustomResolverError) -> Self {
        Self::NoInstantiator {
            type_info: TypeInfo::of::<FailingResolver>(),
        }
    }
}

impl DependencyResolver for FailingResolver {
    type Error = CustomResolverError;

    fn resolve(_: &SyncContainer) -> Result<Self, Self::Error> {
        Err(CustomResolverError)
    }

    #[cfg(feature = "async")]
    async fn resolve_async(_: &AsyncContainer) -> Result<Self, Self::Error> {
        Err(CustomResolverError)
    }
}

struct RecordingInstantiator {
    events: RcThreadSafety<Mutex<Vec<&'static str>>>,
}

impl Clone for RecordingInstantiator {
    fn clone(&self) -> Self {
        self.events.lock().unwrap().push("clone");
        Self {
            events: self.events.clone(),
        }
    }
}

impl Instantiator<(RuntimeDependency<FailingResolver>,)> for RecordingInstantiator {
    type Provides = Combined;
    type Error = InstantiateErrorKind;

    fn instantiate(&mut self, _: (RuntimeDependency<FailingResolver>,)) -> Result<Self::Provides, Self::Error> {
        self.events.lock().unwrap().push("instantiate");
        Ok(Combined([0; 3]))
    }

    fn dependencies() -> BTreeSet<Dependency> {
        BTreeSet::from([Dependency {
            type_info: RuntimeDependency::<FailingResolver>::type_info(),
        }])
    }
}

#[cfg(feature = "async")]
impl AsyncInstantiator<(RuntimeDependency<FailingResolver>,)> for RecordingInstantiator {
    type Provides = Combined;
    type Error = InstantiateErrorKind;

    async fn instantiate(&mut self, _: (RuntimeDependency<FailingResolver>,)) -> Result<Self::Provides, Self::Error> {
        self.events.lock().unwrap().push("instantiate");
        Ok(Combined([0; 3]))
    }

    fn dependencies() -> BTreeSet<Dependency> {
        BTreeSet::from([Dependency {
            type_info: RuntimeDependency::<FailingResolver>::type_info(),
        }])
    }
}

#[derive(Clone)]
#[repr(align(64))]
struct Value(String);
struct Output(String);

#[derive(Clone)]
struct First(usize);
#[derive(Clone)]
struct Second(usize);
struct Combined([usize; 3]);

fn combine(
    first: Inject<First>,
    _: RuntimeDependency<Opaque>,
    second: InjectTransient<Second>,
    repeated: Inject<First>,
) -> Result<Combined, InstantiateErrorKind> {
    Ok(Combined([first.0 .0, second.0 .0, repeated.0 .0]))
}

#[test]
fn one_instantiator_preserves_parameter_positions_in_different_topologies() {
    let first = SyncContainer::new(registry! {
        provide(App, instance(First(11))), provide(App, combine), provide(App, instance(Second(22))),
    });
    let second = SyncContainer::new(registry! {
        provide(App, instance(Second(44))), provide(App, instance(First(33))), provide(App, combine),
    });
    assert_eq!(first.get::<Combined>().unwrap().0, [11, 22, 11]);
    assert_eq!(second.get_transient::<Combined>().unwrap().0, [33, 44, 33]);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_parameter_positions_skip_resolvers_and_preserve_repeats() {
    let container = AsyncContainer::new(async_registry! {
        provide(App, async |first: Inject<First>, _: RuntimeDependency<Opaque>, second: InjectTransient<Second>, repeated: Inject<First>|
            Ok(Combined([first.0.0, second.0.0, repeated.0.0]))),
        extend(registry! { provide(App, instance(Second(22))), provide(App, instance(First(11))) }),
    });
    assert_eq!(container.get::<Combined>().await.unwrap().0, [11, 22, 11]);
}

#[test]
fn custom_resolver_error_converts_and_stops_later_construction() {
    let calls = RcThreadSafety::new(Mutex::new(Vec::new()));
    let first_calls = calls.clone();
    let later_calls = calls.clone();
    let inst_calls = calls.clone();
    let container = SyncContainer::new(registry! {
        provide(App, move || {
            first_calls.lock().unwrap().push("first");
            Ok(First(1))
        }),
        provide(App, move |_: Inject<First>, _: RuntimeDependency<FailingResolver>, _: InjectTransient<Second>| {
            inst_calls.lock().unwrap().push("instantiator");
            Ok(Combined([0; 3]))
        }),
        provide(App, move || {
            later_calls.lock().unwrap().push("later");
            Ok(Second(2))
        }),
    });

    let error = container.get::<Combined>().err().unwrap();
    assert!(matches!(
        error,
        ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(error))
            if matches!(error.as_ref(), ResolveErrorKind::NoInstantiator { type_info }
                if type_info == &TypeInfo::of::<FailingResolver>())
    ));
    assert_eq!(*calls.lock().unwrap(), ["first"]);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_custom_resolver_error_converts_and_stops_later_construction() {
    let calls = RcThreadSafety::new(Mutex::new(Vec::new()));
    let first_calls = calls.clone();
    let later_calls = calls.clone();
    let inst_calls = calls.clone();
    let container = AsyncContainer::new(async_registry! {
        provide(App, move || {
            let calls = first_calls.clone();
            async move {
                calls.lock().unwrap().push("first");
                Ok(First(1))
            }
        }),
        provide(App, move |_: Inject<First>, _: RuntimeDependency<FailingResolver>, _: InjectTransient<Second>| {
            let calls = inst_calls.clone();
            async move {
                calls.lock().unwrap().push("instantiator");
                Ok(Combined([0; 3]))
            }
        }),
        provide(App, move || {
            let calls = later_calls.clone();
            async move {
                calls.lock().unwrap().push("later");
                Ok(Second(2))
            }
        }),
    });

    let error = container.get::<Combined>().await.err().unwrap();
    assert!(matches!(
        error,
        ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(error))
            if matches!(error.as_ref(), ResolveErrorKind::NoInstantiator { type_info }
                if type_info == &TypeInfo::of::<FailingResolver>())
    ));
    assert_eq!(*calls.lock().unwrap(), ["first"]);
}

#[test]
fn failed_dependencies_do_not_clone_the_sync_instantiator() {
    let events = RcThreadSafety::new(Mutex::new(Vec::new()));
    let container = SyncContainer::new(registry! {
        provide(App, RecordingInstantiator { events: events.clone() }),
    });

    let error = container.get::<Combined>().err().unwrap();
    assert!(matches!(
        error,
        ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(error))
            if matches!(error.as_ref(), ResolveErrorKind::NoInstantiator { type_info }
                if type_info == &TypeInfo::of::<FailingResolver>())
    ));
    assert!(events.lock().unwrap().is_empty());
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_instantiator_is_cloned_before_failed_dependency_resolution() {
    let events = RcThreadSafety::new(Mutex::new(Vec::new()));
    let container = AsyncContainer::new(async_registry! {
        provide(App, RecordingInstantiator { events: events.clone() }),
    });

    let error = container.get::<Combined>().await.err().unwrap();
    assert!(matches!(
        error,
        ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(error))
            if matches!(error.as_ref(), ResolveErrorKind::NoInstantiator { type_info }
                if type_info == &TypeInfo::of::<FailingResolver>())
    ));
    assert_eq!(*events.lock().unwrap(), ["clone"]);
}

#[test]
#[should_panic(expected = "Cyclic dependency")]
fn opaque_resolvers_retain_runtime_cycle_validation_for_declared_edges() {
    struct A;
    struct B;
    let _ = SyncContainer::new(registry! {
        provide(App, |_: RuntimeDependency<Opaque>, _: Inject<B>| Ok(A)),
        provide(App, |_: Inject<A>| Ok(B)),
    });
}

#[test]
fn missing_context_boundary_is_a_resolution_error() {
    let app = SyncContainer::new(registry! {
        provide(Request, context::<Value>()),
        provide(Request, |value: Inject<Value>| Ok(Output(value.0.0.clone()))),
    });
    let request = app.enter_build().unwrap();
    assert!(request.get::<Output>().is_err());
}

#[cfg(feature = "async")]
#[tokio::test]
async fn transient_async_dependencies_run_in_their_owning_scope() {
    struct RequestOnly;
    struct OwnerCheck(bool);
    struct Consumer(bool);
    let app = AsyncContainer::new(async_registry! {
        provide(App, async |owner: Inject<SyncContainer>| Ok(OwnerCheck(owner.0.get::<RequestOnly>().is_err()))),
        provide(Request, async |check: InjectTransient<OwnerCheck>| Ok(Consumer(check.0.0))),
        extend(registry! { provide(Request, || Ok(RequestOnly)) }),
    });
    assert!(app.enter_build().unwrap().get::<Consumer>().await.unwrap().0);
}

fn output(
    cached: Inject<Value>,
    _: RuntimeDependency<Opaque>,
    transient: InjectTransient<Value>,
    repeated: Inject<Value>,
) -> Result<Output, InstantiateErrorKind> {
    assert_eq!(cached.0 .0, transient.0 .0);
    assert_eq!(cached.0 .0, repeated.0 .0);
    Ok(Output(transient.0 .0))
}

#[test]
fn ids_remap_after_erasure_composition_and_replacement() {
    let log = RcThreadSafety::new(Mutex::new(Vec::new()));
    let original = log.clone();
    let replacement = log.clone();
    let typed = registry! {
        provide(App, instance(Value("original".into())), finalizer = move |_: RcThreadSafety<Value>| {
            original.lock().unwrap().push("original");
        }),
        provide(Request, output),
    }
    .into_registry();
    let registry = dynamic_registry! {
        provide(App, instance(55u32)),
        extend(typed, dynamic_registry! {
            provide(App, instance(Value("replacement".into())), config = Config { cache_provides: false },
                finalizer = move |_: RcThreadSafety<Value>| replacement.lock().unwrap().push("replacement")),
        }),
    };
    let app = SyncContainer::new(registry);
    let request = app.clone().enter_build().unwrap();
    drop(app);
    assert_eq!(request.get_transient::<Output>().unwrap().0, "replacement");
    drop(request);
    assert_eq!(*log.lock().unwrap(), ["replacement", "replacement"]);
}

#[test]
fn runtime_import_and_context_use_original_lifecycle() {
    let dynamic = dynamic_registry! { provide(App, instance(Value("runtime".into()))) };
    let app = SyncContainer::new(registry! {
        provide(App, runtime::<Value>()),
        provide(Request, output),
        extend(dynamic),
    });
    let request = app.enter_build().unwrap();
    assert_eq!(request.get::<Output>().unwrap().0, "runtime");
    let app = SyncContainer::new(registry! {
        provide(Request, context::<Value>()),
        provide(Request, |value: Inject<Value>| Ok::<_, InstantiateErrorKind>(Output(value.0.0.clone()))),
    });
    let mut context = Context::new();
    context.insert(Value("context".into()));
    let request = app.enter().with_context(context).build().unwrap();
    assert_eq!(request.get::<Output>().unwrap().0, "context");
    assert!(request.get_transient::<Value>().is_err());
}

#[test]
fn replacement_removes_cycle_from_final_composition() {
    struct A;
    struct B;
    let fragment = registry! {
        provide(App, |_: Inject<B>| Ok::<_, InstantiateErrorKind>(A)),
        provide(App, |_: Inject<A>| Ok::<_, InstantiateErrorKind>(B)),
    };
    let overrides = dynamic_registry! { provide(App, || Ok::<_, InstantiateErrorKind>(A)) };
    let container = SyncContainer::new(registry! { extend(fragment, overrides) });
    container.get::<B>().unwrap();
}

#[test]
#[should_panic(expected = "Cyclic dependency")]
fn replacement_introduces_cycle_in_effective_graph() {
    struct A;
    struct B;
    let fragment = registry! {
        provide(App, || Ok::<_, InstantiateErrorKind>(A)),
        provide(App, |_: Inject<A>| Ok::<_, InstantiateErrorKind>(B)),
    };
    let overrides = dynamic_registry! { provide(App, |_: Inject<B>| Ok::<_, InstantiateErrorKind>(A)) };
    let _ = SyncContainer::new(registry! { extend(fragment, overrides) });
}

#[cfg(feature = "async")]
#[tokio::test]
async fn mixed_async_runtime_replacements_and_parameter_order() {
    let log = RcThreadSafety::new(Mutex::new(0usize));
    let finalized = log.clone();
    let typed = async_registry! {
        provide(App, async || Ok::<_, InstantiateErrorKind>(Value("old".into()))),
        provide(Request, async |
            cached: Inject<Value>,
            _: RuntimeDependency<Opaque>,
            transient: InjectTransient<Value>,
            repeated: Inject<Value>,
            number: Inject<u32>,
        | {
            assert_eq!(cached.0.0, transient.0.0);
            assert_eq!(transient.0.0, repeated.0.0);
            assert_eq!(*number.0, 7);
            Ok::<_, InstantiateErrorKind>(Output(transient.0.0))
        }),
        extend(registry! { provide(App, instance(7u32)) }),
    };
    let replacements = dynamic_async_registry! {
        provide(App, async || Ok::<_, InstantiateErrorKind>(Value("async".into())), config = Config { cache_provides: false },
            finalizer = move |_: RcThreadSafety<Value>| {
                let finalized = finalized.clone();
                async move { *finalized.lock().unwrap() += 1; }
            }),
    };
    let app = AsyncContainer::new(async_registry! { extend(typed, replacements) });
    let request = app.clone().enter_build().unwrap();
    assert_eq!(request.get_transient::<Output>().await.unwrap().0, "async");
    request.close().await;
    app.close().await;
    assert_eq!(*log.lock().unwrap(), 2);
}

#[test]
fn erased_fragment_defers_validation_until_after_replacement() {
    struct A;
    struct B;
    let fragment = registry! {
        provide(App, |_: Inject<B>| Ok::<_, InstantiateErrorKind>(A)),
        provide(App, |_: Inject<A>| Ok::<_, InstantiateErrorKind>(B)),
    }
    .into_registry();
    let registry = dynamic_registry! {
        extend(fragment, dynamic_registry! { provide(App, || Ok::<_, InstantiateErrorKind>(A)) }),
    };
    SyncContainer::new(registry).get::<B>().unwrap();
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_replacement_of_sync_provider_preserves_native_namespaces_and_context() {
    struct SyncConsumer(String);
    struct AsyncConsumer(String);
    let sync = registry! {
        provide(App, instance(Value("sync".into()))),
        provide(Request, |value: Inject<Value>| Ok::<_, InstantiateErrorKind>(SyncConsumer(value.0.0.clone()))),
    };
    let typed = async_registry! {
        provide(Request, async |value: Inject<Value>| Ok::<_, InstantiateErrorKind>(AsyncConsumer(value.0.0.clone()))),
        extend(sync),
    };
    let replacement = dynamic_async_registry! {
        provide(Request, async || Ok::<_, InstantiateErrorKind>(Value("async".into()))),
    };
    let app = AsyncContainer::new(async_registry! { extend(typed, replacement) });
    let request = app.clone().enter_build().unwrap();
    assert_eq!(request.get::<AsyncConsumer>().await.unwrap().0, "async");
    assert_eq!(request.get::<SyncConsumer>().await.unwrap().0, "sync");
    let mut context = Context::new();
    context.insert(Value("context".into()));
    let request = app.enter().with_context(context).build().unwrap();
    assert_eq!(request.get::<AsyncConsumer>().await.unwrap().0, "context");
    assert_eq!(request.get::<SyncConsumer>().await.unwrap().0, "context");
}

#[test]
fn inline_closures_infer_the_original_error_type() {
    let container = SyncContainer::new(registry! {
        provide(App, || Ok(7u32)),
        provide(App, |number: Inject<u32>| Ok(number.0.to_string())),
    });
    assert_eq!(&*container.get::<String>().unwrap(), "7");
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_inline_closures_infer_the_original_error_type() {
    let container = AsyncContainer::new(async_registry! {
        provide(App, async || Ok(7u32)),
        provide(App, async |number: Inject<u32>| Ok(number.0.to_string())),
    });
    assert_eq!(&*container.get::<String>().await.unwrap(), "7");
}

#[test]
#[should_panic(expected = "Cyclic dependency")]
fn fully_replaced_typed_composition_still_validates_the_effective_graph() {
    struct A;
    struct B;
    let original = registry! { provide(App, || Ok(A)), provide(App, || Ok(B)) };
    let left = dynamic_registry! { provide(App, |_: Inject<B>| Ok(A)) };
    let right = dynamic_registry! { provide(App, |_: Inject<A>| Ok(B)) };
    let _ = SyncContainer::new(registry! { extend(original, left, right) });
}

#[cfg(feature = "async")]
#[test]
#[should_panic(expected = "Cyclic dependency")]
fn fully_replaced_async_composition_still_validates_the_effective_graph() {
    struct A;
    struct B;
    let original = async_registry! { provide(App, async || Ok(A)), provide(App, async || Ok(B)) };
    let left = dynamic_async_registry! { provide(App, async |_: Inject<B>| Ok(A)) };
    let right = dynamic_async_registry! { provide(App, async |_: Inject<A>| Ok(B)) };
    let _ = AsyncContainer::new(async_registry! { extend(original, left, right) });
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_import_and_context_declarations_link_to_native_values() {
    let imported = dynamic_async_registry! { provide(App, async || Ok(Value("imported".into()))) };
    let app = AsyncContainer::new(async_registry! {
        provide(App, runtime::<Value>()),
        provide(Request, context::<u32>()),
        provide(Request, async |value: Inject<Value>, number: Inject<u32>| Ok(Output(format!("{}-{}", value.0.0, number.0)))),
        extend(imported),
    });
    let mut context = Context::new();
    context.insert(42u32);
    let request = app.enter().with_context(context).build().unwrap();
    assert_eq!(request.get::<Output>().await.unwrap().0, "imported-42");
    assert!(request.get_transient::<u32>().await.is_err());
}
