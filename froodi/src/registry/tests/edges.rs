extern crate std;

#[cfg(feature = "async")]
use alloc::format;
use alloc::{
    collections::BTreeSet,
    string::{String, ToString as _},
    vec::Vec,
};

#[cfg(feature = "async")]
use crate::async_impl::{Container as AsyncContainer, Instantiator as AsyncInstantiator};
use crate::{
    declare,
    errors::InstantiatorErrorKind,
    instance,
    instantiator::RegistrationInstantiator,
    registry,
    registry::RegistrationId,
    registry::CYCLE_CHECKS,
    utils::thread_safety::RcThreadSafety,
    Config, Container as SyncContainer, Context,
    DefaultScope::{App, Request},
    Dependency, DependencyResolver, Inject, InjectCustom, InjectTransient, InstantiateErrorKind, Instantiator, ResolveErrorKind, TypeInfo,
};
use core::cell::Cell;
use std::sync::Mutex;

#[cfg(feature = "async")]
use crate::utils::thread_safety::SendSafety;
#[cfg(feature = "async")]
use alloc::boxed::Box;
#[cfg(feature = "async")]
use core::{
    future::Future,
    pin::Pin,
    task::{Context as TaskContext, Poll, Waker},
};
use std::panic::{catch_unwind, AssertUnwindSafe};

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

struct MutableResolver(Cell<u32>);

impl DependencyResolver for MutableResolver {
    type Error = ResolveErrorKind;

    fn resolve(_: &SyncContainer) -> Result<Self, Self::Error> {
        Ok(Self(Cell::new(7)))
    }

    #[cfg(feature = "async")]
    async fn resolve_async(_: &AsyncContainer) -> Result<Self, Self::Error> {
        Ok(Self(Cell::new(7)))
    }
}

#[test]
fn linked_adapter_does_not_require_dependencies_to_be_sync() {
    let container = SyncContainer::new(registry! {
        provide(App, |InjectCustom(value): InjectCustom<MutableResolver>| Ok(value.0.get())),
    });
    assert_eq!(container.get_transient::<u32>().unwrap(), 7);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_linked_adapter_does_not_require_dependencies_to_be_sync() {
    let container = AsyncContainer::new(registry! {
        provide(App, async |InjectCustom(value): InjectCustom<MutableResolver>| Ok(value.0.get())),
    });
    assert_eq!(container.get_transient::<u32>().await.unwrap(), 7);
}

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

impl Instantiator<(InjectCustom<FailingResolver>,)> for RecordingInstantiator {
    type Provides = Combined;
    type Error = InstantiateErrorKind;

    fn instantiate(&mut self, _: (InjectCustom<FailingResolver>,)) -> Result<Self::Provides, Self::Error> {
        self.events.lock().unwrap().push("instantiate");
        Ok(Combined([0; 3]))
    }

    fn dependencies() -> BTreeSet<Dependency> {
        BTreeSet::from([Dependency {
            type_info: InjectCustom::<FailingResolver>::type_info(),
        }])
    }
}

#[cfg(feature = "async")]
#[derive(Clone)]
struct AsyncRecordingInstantiator(RecordingInstantiator);

#[cfg(feature = "async")]
impl AsyncInstantiator<(InjectCustom<FailingResolver>,)> for AsyncRecordingInstantiator {
    type Provides = Combined;
    type Error = InstantiateErrorKind;

    async fn instantiate(&mut self, _: (InjectCustom<FailingResolver>,)) -> Result<Self::Provides, Self::Error> {
        self.0.events.lock().unwrap().push("instantiate");
        Ok(Combined([0; 3]))
    }

    fn dependencies() -> BTreeSet<Dependency> {
        BTreeSet::from([Dependency {
            type_info: InjectCustom::<FailingResolver>::type_info(),
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
    _: InjectCustom<Opaque>,
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
    assert_eq!(
        first.inner.registry.indexed.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>(),
        [
            TypeInfo::of::<First>(),
            TypeInfo::of::<Combined>(),
            TypeInfo::of::<Second>(),
            TypeInfo::of::<SyncContainer>(),
            #[cfg(feature = "async")]
            TypeInfo::of::<AsyncContainer>(),
        ]
    );
    let RegistrationInstantiator::Linked(executor) = &first.inner.registry.get(&TypeInfo::of::<Combined>()).unwrap().instantiator else {
        panic!("expected linked instantiator");
    };
    assert_eq!(executor.edges.iter().map(|id| id.index()).collect::<Vec<_>>(), [0, 2, 0]);
}

#[test]
fn fixed_plan_keeps_linked_edge_storage() {
    use crate::registry::{frontend::prepare, IntoRegistry};

    let (registry, plan) = registry! {
        provide(App, instance(First(11))),
        provide(App, combine),
        provide(App, instance(Second(22))),
    }
    .materialize();
    let RegistrationInstantiator::Linked(executor) = &registry.get(&TypeInfo::of::<Combined>()).unwrap().instantiator else {
        panic!("expected linked instantiator");
    };
    let edges = executor.edges.clone();
    assert!(plan.is_some());
    let registry = prepare(registry, false, plan);
    let RegistrationInstantiator::Linked(executor) = &registry.get(&TypeInfo::of::<Combined>()).unwrap().instantiator else {
        panic!("expected linked instantiator");
    };
    assert!(RcThreadSafety::ptr_eq(&edges, &executor.edges));
}

#[test]
fn fixed_plan_preserves_unused_duplicate_registration_precedence() {
    let container = SyncContainer::new(registry! {
        provide(App, instance(7u32)),
        provide(App, instance(9u32)),
    });
    assert_eq!(*container.get::<u32>().unwrap(), 9);
    assert_eq!(container.get_transient::<u32>().unwrap(), 9);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn fixed_async_plan_shares_ids_with_sync_dependencies() {
    let container = AsyncContainer::new(registry! {
        provide(App, async |first: Inject<First>, _: InjectCustom<Opaque>, second: InjectTransient<Second>, repeated: Inject<First>| {
            Ok(Combined([first.0.0, second.0.0, repeated.0.0]))
        }),
        extend(registry! {
            provide(App, instance(First(11))),
            provide(App, |first: Inject<First>| Ok(Second(first.0.0 * 2))),
        }),
    });
    let keys = [
        TypeInfo::of::<Combined>(),
        TypeInfo::of::<First>(),
        TypeInfo::of::<Second>(),
        TypeInfo::of::<SyncContainer>(),
        TypeInfo::of::<AsyncContainer>(),
    ];
    assert_eq!(
        container
            .inner
            .registry
            .indexed
            .iter()
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>(),
        keys
    );
    assert_eq!(
        container
            .sync
            .inner
            .registry
            .indexed
            .iter()
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>(),
        keys
    );
    let RegistrationInstantiator::Linked(executor) = &container.sync.inner.registry.get(&TypeInfo::of::<Second>()).unwrap().instantiator
    else {
        panic!("expected linked instantiator");
    };
    assert_eq!(executor.edges.iter().map(|id| id.index()).collect::<Vec<_>>(), [1]);
    assert_eq!(container.get::<Combined>().await.unwrap().0, [11, 22, 11]);
    assert_eq!(container.sync.get::<Second>().unwrap().0, 22);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn fixed_async_plan_preserves_native_namespace_precedence_without_edges() {
    let container = AsyncContainer::new(registry! {
        provide(App, async || Ok::<_, InstantiateErrorKind>(9u32)),
        extend(registry! { provide(App, instance(7u32)) }),
    });
    assert_eq!(*container.get::<u32>().await.unwrap(), 9);
    assert_eq!(container.get_transient::<u32>().await.unwrap(), 9);
    assert_eq!(*container.sync.get::<u32>().unwrap(), 7);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_parameter_positions_skip_resolvers_and_preserve_repeats() {
    let container = AsyncContainer::new(registry! {
        provide(App, async |first: Inject<First>, _: InjectCustom<Opaque>, second: InjectTransient<Second>, repeated: Inject<First>|
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
        provide(App, move |_: Inject<First>, _: InjectCustom<FailingResolver>, _: InjectTransient<Second>| {
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
    let container = AsyncContainer::new(registry! {
        provide(App, move || {
            let calls = first_calls.clone();
            async move {
                calls.lock().unwrap().push("first");
                Ok(First(1))
            }
        }),
        provide(App, move |_: Inject<First>, _: InjectCustom<FailingResolver>, _: InjectTransient<Second>| {
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
    let container = AsyncContainer::new(registry! {
        provide(App, AsyncRecordingInstantiator(RecordingInstantiator { events: events.clone() })),
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
fn opaque_composition_still_checks_cycles_at_runtime() {
    CYCLE_CHECKS.with(|checks| checks.set(0));
    let container = SyncContainer::new(registry! {
        provide(App, |_: InjectCustom<Opaque>, value: Inject<u32>, repeated: Inject<u32>| {
            assert!(RcThreadSafety::ptr_eq(&value.0, &repeated.0));
            Ok(value.0.to_string())
        }),
        provide(App, instance(7u32)),
    });
    assert_eq!(&*container.get::<String>().unwrap(), "7");
    assert_eq!(CYCLE_CHECKS.with(Cell::get), 1);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn opaque_mixed_composition_checks_the_effective_graph_once() {
    CYCLE_CHECKS.with(|checks| checks.set(0));
    let container = AsyncContainer::new(registry! {
        provide(App, async |_: InjectCustom<Opaque>, value: Inject<u32>, repeated: Inject<u32>| {
            assert!(RcThreadSafety::ptr_eq(&value.0, &repeated.0));
            Ok(value.0.to_string())
        }),
        extend(registry! { provide(App, instance(7u32)) }),
    });
    assert_eq!(&*container.get::<String>().await.unwrap(), "7");
    assert_eq!(CYCLE_CHECKS.with(Cell::get), 1);
}

#[test]
#[should_panic(expected = "Cyclic dependency")]
fn runtime_composition_defers_known_cycles_with_opaque_parameters() {
    struct A;
    struct B;
    let _ = SyncContainer::new(registry! {
        provide(App, |_: InjectCustom<Opaque>, _: Inject<B>| Ok(A)),
        provide(App, |_: Inject<A>| Ok(B)),
        extend(native_registry! { provide(App, instance(7u32)) }),
    });
}

#[test]
fn closed_composition_does_not_repeat_runtime_cycle_checks() {
    CYCLE_CHECKS.with(|checks| checks.set(0));
    let fragment = registry! {
        provide(App, |number: Inject<u32>| Ok(number.0.to_string())),
    };
    let container = SyncContainer::new(registry! {
        provide(App, instance(7u32)),
        extend(fragment),
    });
    assert_eq!(&*container.get::<String>().unwrap(), "7");
    let explicit = SyncContainer::new_with_start_scope(registry! { provide(App, instance(9u32)) }, App);
    assert_eq!(*explicit.get::<u32>().unwrap(), 9);
    assert_eq!(CYCLE_CHECKS.with(Cell::get), 0);

    container.inner.registry.validate().unwrap();
    assert_eq!(CYCLE_CHECKS.with(Cell::get), 1);
}

#[test]
fn closed_composition_still_checks_scopes_at_runtime() {
    CYCLE_CHECKS.with(|checks| checks.set(0));
    let error = catch_unwind(|| {
        SyncContainer::new(registry! {
            provide(App, |_: Inject<u32>| Ok::<_, InstantiateErrorKind>(true)),
            provide(Request, instance(7u32)),
        })
    })
    .err()
    .unwrap();
    assert!(error.downcast_ref::<String>().unwrap().contains("Unreachable dependency"));
    assert_eq!(CYCLE_CHECKS.with(Cell::get), 0);
}

#[test]
fn erasure_retains_runtime_cycle_checks_for_both_constructors() {
    CYCLE_CHECKS.with(|checks| checks.set(0));
    let native = registry! { provide(App, instance(7u32)) }.into_registry();
    assert_eq!(CYCLE_CHECKS.with(Cell::get), 0);
    SyncContainer::new(native.clone()).get::<u32>().unwrap();
    assert_eq!(CYCLE_CHECKS.with(Cell::get), 1);
    SyncContainer::new_with_start_scope(native, App).get::<u32>().unwrap();
    assert_eq!(CYCLE_CHECKS.with(Cell::get), 2);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn closed_mixed_composition_skips_runtime_cycle_checks() {
    CYCLE_CHECKS.with(|checks| checks.set(0));
    let sync = registry! { provide(App, instance(7u32)) };
    let container = AsyncContainer::new(registry! {
        extend(sync),
    });
    assert_eq!(*container.get::<u32>().await.unwrap(), 7);
    let explicit = AsyncContainer::new_with_start_scope(
        registry! {
            provide(App, async |number: Inject<u32>| Ok(number.0.to_string())),
            extend(registry! { provide(App, instance(9u32)) }),
        },
        App,
    );
    assert_eq!(&*explicit.get::<String>().await.unwrap(), "9");
    assert_eq!(CYCLE_CHECKS.with(Cell::get), 0);

    explicit.inner.registry.validate().unwrap();
    assert_eq!(CYCLE_CHECKS.with(Cell::get), 1);
}

#[cfg(feature = "async")]
#[test]
fn closed_mixed_composition_still_checks_both_execution_scopes() {
    CYCLE_CHECKS.with(|checks| checks.set(0));
    for sync_dependency in [false, true] {
        let error = catch_unwind(|| {
            if sync_dependency {
                AsyncContainer::new(registry! {
                    provide(App, async |_: Inject<u32>| Ok::<_, InstantiateErrorKind>(true)),
                    extend(registry! { provide(Request, instance(7u32)) }),
                })
            } else {
                AsyncContainer::new(registry! {
                    provide(App, async |_: Inject<u32>| Ok::<_, InstantiateErrorKind>(true)),
                    provide(Request, async || Ok::<_, InstantiateErrorKind>(7u32)),
                })
            }
        })
        .err()
        .unwrap();
        let message = error.downcast_ref::<String>().unwrap().to_lowercase();
        assert!(message.contains("unreachable dependency"));
        assert_eq!(CYCLE_CHECKS.with(Cell::get), 0);
    }
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_erasure_retains_runtime_cycle_checks_for_both_constructors() {
    CYCLE_CHECKS.with(|checks| checks.set(0));
    let native = registry! {
        provide(App, async |number: Inject<u32>| Ok(number.0.to_string())),
        extend(registry! { provide(App, instance(7u32)) }),
    }
    .into_registry();
    assert_eq!(CYCLE_CHECKS.with(Cell::get), 0);
    AsyncContainer::new(native.clone()).get::<String>().await.unwrap();
    assert_eq!(CYCLE_CHECKS.with(Cell::get), 1);
    AsyncContainer::new_with_start_scope(native, App).get::<String>().await.unwrap();
    assert_eq!(CYCLE_CHECKS.with(Cell::get), 2);
}

#[test]
fn missing_context_boundary_is_a_resolution_error() {
    let app = SyncContainer::new(registry! {
        provide(Request, declare::<Value>()),
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
    let app = AsyncContainer::new(registry! {
        provide(App, async |owner: Inject<SyncContainer>| Ok(OwnerCheck(owner.0.get::<RequestOnly>().is_err()))),
        provide(Request, async |check: InjectTransient<OwnerCheck>| Ok(Consumer(check.0.0))),
        extend(registry! { provide(Request, || Ok(RequestOnly)) }),
    });
    assert!(app.enter_build().unwrap().get::<Consumer>().await.unwrap().0);
}

fn output(
    cached: Inject<Value>,
    _: InjectCustom<Opaque>,
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
    let registry = native_registry! {
        provide(App, instance(55u32)),
        extend(typed, native_registry! {
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
fn runtime_fragments_override_later_typed_providers_in_fragment_order() {
    let first = native_registry! { provide(App, instance(Value("first".into()))) };
    let last = native_registry! { provide(App, instance(Value("last".into()))) };
    let container = SyncContainer::new(registry! {
        extend(first, last),
        provide(App, instance(Value("typed".into()))),
        provide(App, output),
    });

    assert_eq!(container.get::<Output>().unwrap().0, "last");
    assert_eq!(container.get_transient::<Value>().unwrap().0, "last");
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_runtime_fragments_override_later_typed_providers_in_fragment_order() {
    let first = native_async_registry! { provide(App, async || Ok(Value("first".into()))) };
    let last = native_async_registry! { provide(App, async || Ok(Value("last".into()))) };
    let container = AsyncContainer::new(registry! {
        extend(first, last),
        provide(App, async || Ok(Value("typed".into()))),
        provide(App, async |cached: Inject<Value>, transient: InjectTransient<Value>| {
            assert_eq!(cached.0.0, transient.0.0);
            Ok(Output(transient.0.0))
        }),
    });

    assert_eq!(container.get::<Output>().await.unwrap().0, "last");
    assert_eq!(container.get_transient::<Value>().await.unwrap().0, "last");
}

#[test]
fn runtime_import_and_context_use_original_lifecycle() {
    let dynamic = native_registry! { provide(App, instance(Value("runtime".into()))) };
    let app = SyncContainer::new(registry! {
        provide(App, declare::<Value>()),
        provide(Request, output),
        extend(dynamic),
    });
    let request = app.enter_build().unwrap();
    assert_eq!(request.get::<Output>().unwrap().0, "runtime");
    let app = SyncContainer::new(registry! {
        provide(Request, declare::<Value>()),
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
    let overrides = native_registry! { provide(App, || Ok::<_, InstantiateErrorKind>(A)) };
    let container = SyncContainer::new(registry! { extend(fragment, overrides) });
    container.get::<B>().unwrap();
}

#[test]
fn opaque_cycle_can_be_removed_by_replacement_before_or_after_erasure() {
    struct A;
    struct B;
    for erased in [false, true] {
        let fragment = registry! {
            provide(App, |_: InjectCustom<Opaque>, _: Inject<B>| Ok::<_, InstantiateErrorKind>(A)),
            provide(App, |_: InjectTransient<A>| Ok::<_, InstantiateErrorKind>(B)),
        };
        let overrides = native_registry! { provide(App, || Ok::<_, InstantiateErrorKind>(A)) };
        let container = if erased {
            SyncContainer::new(native_registry! { extend(fragment.into_registry(), overrides) })
        } else {
            SyncContainer::new(registry! { extend(fragment, overrides) })
        };
        container.get::<B>().unwrap();
    }
}

#[cfg(feature = "async")]
#[tokio::test]
async fn opaque_async_cycle_can_be_removed_before_or_after_erasure() {
    struct A;
    struct B;
    for erased in [false, true] {
        let fragment = registry! {
            provide(App, async |_: InjectCustom<Opaque>, _: Inject<B>| Ok::<_, InstantiateErrorKind>(A)),
            provide(App, async |_: InjectTransient<A>| Ok::<_, InstantiateErrorKind>(B)),
        };
        let overrides = native_async_registry! { provide(App, async || Ok::<_, InstantiateErrorKind>(A)) };
        let container = if erased {
            AsyncContainer::new(native_async_registry! { extend(fragment.into_registry(), overrides) })
        } else {
            AsyncContainer::new(registry! { extend(fragment, overrides) })
        };
        container.get::<B>().await.unwrap();
    }
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
    let overrides = native_registry! {
        provide(App, |_: Inject<B>| Ok::<_, InstantiateErrorKind>(A)),
        provide(App, declare::<B>()),
    };
    let _ = SyncContainer::new(registry! { extend(fragment, overrides) });
}

#[cfg(feature = "async")]
#[tokio::test]
async fn mixed_async_runtime_replacements_and_parameter_order() {
    let log = RcThreadSafety::new(Mutex::new(0usize));
    let finalized = log.clone();
    let typed = registry! {
        provide(App, async || Ok::<_, InstantiateErrorKind>(Value("old".into()))),
        provide(Request, async |
            cached: Inject<Value>,
            _: InjectCustom<Opaque>,
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
    let replacements = native_async_registry! {
        provide(App, async || Ok::<_, InstantiateErrorKind>(Value("async".into())), config = Config { cache_provides: false },
            finalizer = move |_: RcThreadSafety<Value>| {
                let finalized = finalized.clone();
                async move { *finalized.lock().unwrap() += 1; }
            }),
    };
    let app = AsyncContainer::new(registry! { extend(typed, replacements) });
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
    let registry = native_registry! {
        extend(fragment, native_registry! { provide(App, || Ok::<_, InstantiateErrorKind>(A)) }),
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
    let typed = registry! {
        provide(Request, async |value: Inject<Value>| Ok::<_, InstantiateErrorKind>(AsyncConsumer(value.0.0.clone()))),
        extend(sync),
    };
    let replacement = native_async_registry! {
        provide(Request, async || Ok::<_, InstantiateErrorKind>(Value("async".into()))),
    };
    let app = AsyncContainer::new(registry! { extend(typed, replacement) });
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
    let container = AsyncContainer::new(registry! {
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
    let left = native_registry! { provide(App, |_: Inject<B>| Ok(A)), provide(App, declare::<B>()) };
    let right = native_registry! { provide(App, |_: Inject<A>| Ok(B)), provide(App, declare::<A>()) };
    let _ = SyncContainer::new(registry! { extend(original, left, right) });
}

#[cfg(feature = "async")]
#[test]
#[should_panic(expected = "Cyclic dependency")]
fn fully_replaced_async_composition_still_validates_the_effective_graph() {
    struct A;
    struct B;
    let original = registry! { provide(App, async || Ok(A)), provide(App, async || Ok(B)) };
    let left = native_async_registry! { provide(App, async |_: Inject<B>| Ok(A)), provide(App, declare::<B>()) };
    let right = native_async_registry! { provide(App, async |_: Inject<A>| Ok(B)), provide(App, declare::<A>()) };
    let _ = AsyncContainer::new(registry! { extend(original, left, right) });
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_import_and_context_declarations_link_to_native_values() {
    let imported = native_async_registry! { provide(App, async || Ok(Value("imported".into()))) };
    let app = AsyncContainer::new(registry! {
        provide(App, declare::<Value>()),
        provide(Request, declare::<u32>()),
        provide(Request, async |value: Inject<Value>, number: Inject<u32>| Ok(Output(format!("{}-{}", value.0.0, number.0)))),
        extend(imported),
    });
    let mut context = Context::new();
    context.insert(42u32);
    let request = app.enter().with_context(context).build().unwrap();
    assert_eq!(request.get::<Output>().await.unwrap().0, "imported-42");
    assert!(request.get_transient::<u32>().await.is_err());
}

mod custom_scope {
    #[cfg(feature = "async")]
    use crate::async_impl::Container as AsyncContainer;
    use crate::{
        instance, registry, utils::thread_safety::RcThreadSafety, Container, Context, Inject, ResolveErrorKind, Scope, ScopeData, Scopes,
    };

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
    enum MyScope {
        Boot,
        Work,
        Task,
    }

    impl From<MyScope> for ScopeData {
        fn from(scope: MyScope) -> Self {
            Self {
                priority: scope.priority(),
                name: scope.name(),
                is_skipped_by_default: scope.is_skipped_by_default(),
            }
        }
    }

    impl Scope for MyScope {
        fn name(&self) -> &'static str {
            match self {
                MyScope::Boot => "boot",
                MyScope::Work => "work",
                MyScope::Task => "task",
            }
        }

        fn priority(&self) -> u8 {
            *self as u8
        }

        fn is_skipped_by_default(&self) -> bool {
            matches!(self, MyScope::Boot)
        }
    }

    impl Scopes<2> for MyScope {
        type Scope = Self;

        fn all() -> (Self, [Self; 2]) {
            (MyScope::Boot, [MyScope::Work, MyScope::Task])
        }
    }

    struct Wide(u32);
    struct Narrow(RcThreadSafety<Wide>);

    #[test]
    fn indexed_dependencies_use_their_owning_custom_scope() {
        let container = Container::new(registry! {
            scope(MyScope::Work) [
                provide(instance(7u32)),
                provide(|number: Inject<u32>| Ok(Wide(*number.0))),
            ],
            provide(MyScope::Task, |wide: Inject<Wide>| Ok(Narrow(wide.0))),
        });

        match container.get::<Narrow>() {
            Err(ResolveErrorKind::NoAccessible {
                expected_scope_data,
                actual_scope_data,
            }) => assert_eq!((expected_scope_data.name, actual_scope_data.name), ("task", "work")),
            other => panic!("expected NoAccessible, got {:?}", other.err()),
        }

        let mut context = Context::new();
        context.insert(99u32);
        let task = container
            .clone()
            .enter()
            .with_scope(MyScope::Task)
            .with_context(context)
            .build()
            .unwrap();
        let narrow = task.get::<Narrow>().unwrap();
        assert_eq!(narrow.0 .0, 7);
        assert!(RcThreadSafety::ptr_eq(&narrow.0, &container.get::<Wide>().unwrap()));
        assert_eq!(task.get_transient::<Wide>().unwrap().0, 7);
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn async_indexed_dependencies_use_their_owning_custom_scope() {
        let container = AsyncContainer::new(registry! {
            scope(MyScope::Work) [
                provide(async || Ok(7u32)),
                provide(async |number: Inject<u32>| Ok(Wide(*number.0))),
            ],
            provide(MyScope::Task, async |wide: Inject<Wide>| Ok(Narrow(wide.0))),
        });

        match container.get::<Narrow>().await {
            Err(ResolveErrorKind::NoAccessible {
                expected_scope_data,
                actual_scope_data,
            }) => assert_eq!((expected_scope_data.name, actual_scope_data.name), ("task", "work")),
            other => panic!("expected NoAccessible, got {:?}", other.err()),
        }

        let mut context = Context::new();
        context.insert(99u32);
        let task = container
            .clone()
            .enter()
            .with_scope(MyScope::Task)
            .with_context(context)
            .build()
            .unwrap();
        let narrow = task.get::<Narrow>().await.unwrap();
        assert_eq!(narrow.0 .0, 7);
        assert!(RcThreadSafety::ptr_eq(&narrow.0, &container.get::<Wide>().await.unwrap()));
        assert_eq!(task.get_transient::<Wide>().await.unwrap().0, 7);
    }
}

#[derive(Default)]
struct AdapterCounts {
    clones: usize,
    clone_drops: usize,
    owner_drops: usize,
    calls: usize,
    value_drops: usize,
    state_drops: usize,
    finalizers: usize,
    #[cfg(feature = "async")]
    futures: usize,
    #[cfg(feature = "async")]
    future_drops: usize,
    #[cfg(feature = "async")]
    polls: usize,
    #[cfg(feature = "async")]
    ready: bool,
}

struct AdapterState(RcThreadSafety<Mutex<AdapterCounts>>);

impl Drop for AdapterState {
    fn drop(&mut self) {
        self.0.lock().unwrap().state_drops += 1;
    }
}

#[repr(align(256))]
struct AlignedInst {
    counts: RcThreadSafety<Mutex<AdapterCounts>>,
    state: RcThreadSafety<AdapterState>,
    word: u64,
    original: bool,
}

impl Clone for AlignedInst {
    fn clone(&self) -> Self {
        assert!((self as *const Self).is_aligned());
        assert_eq!(self.word, 0x1234_5678_9abc_def0);
        self.counts.lock().unwrap().clones += 1;
        Self {
            counts: self.counts.clone(),
            state: self.state.clone(),
            word: self.word,
            original: false,
        }
    }
}

impl Drop for AlignedInst {
    fn drop(&mut self) {
        assert_eq!(self.word, 0x1234_5678_9abc_def0);
        let mut counts = self.counts.lock().unwrap();
        if self.original {
            counts.owner_drops += 1;
        } else {
            counts.clone_drops += 1;
        }
    }
}

struct AlignedValue {
    counts: RcThreadSafety<Mutex<AdapterCounts>>,
    value: usize,
}

impl Drop for AlignedValue {
    fn drop(&mut self) {
        self.counts.lock().unwrap().value_drops += 1;
    }
}

impl Instantiator<()> for AlignedInst {
    type Provides = AlignedValue;
    type Error = InstantiateErrorKind;

    fn instantiate(&mut self, _: ()) -> Result<Self::Provides, Self::Error> {
        self.counts.lock().unwrap().calls += 1;
        Ok(AlignedValue {
            counts: self.counts.clone(),
            value: 73,
        })
    }

    fn dependencies() -> BTreeSet<Dependency> {
        BTreeSet::new()
    }
}

#[test]
fn aligned_inst_owner_survives_registry_moves_and_clones() {
    let counts = RcThreadSafety::new(Mutex::new(AdapterCounts::default()));
    let state = RcThreadSafety::new(AdapterState(counts.clone()));
    let weak = RcThreadSafety::downgrade(&state);
    let finalized = counts.clone();
    let native = registry! {
        provide(Request, AlignedInst {
            counts: counts.clone(),
            state,
            word: 0x1234_5678_9abc_def0,
            original: true,
        }, finalizer = move |_: RcThreadSafety<AlignedValue>| finalized.lock().unwrap().finalizers += 1),
    }
    .into_registry();
    let mut moved = Some(native);
    let native = moved.take().unwrap();
    let copied = native.clone();
    assert_eq!(counts.lock().unwrap().clones, 0);

    let app = SyncContainer::new(native);
    let request = app.clone().enter_build().unwrap();
    drop(app);
    let first = request.get::<AlignedValue>().unwrap();
    assert_eq!(first.value, 73);
    assert!(weak.upgrade().is_some());
    assert!(RcThreadSafety::ptr_eq(&first, &request.get::<AlignedValue>().unwrap()));
    let transient = request.get_transient::<AlignedValue>().unwrap();
    assert_eq!(transient.value, 73);
    drop(transient);

    let second = SyncContainer::new(copied).enter_build().unwrap();
    drop(second.get::<AlignedValue>().unwrap());
    {
        let counts = counts.lock().unwrap();
        assert_eq!((counts.calls, counts.clones, counts.clone_drops), (3, 3, 3));
        assert_eq!((counts.owner_drops, counts.value_drops, counts.finalizers), (0, 1, 0));
    }

    drop(first);
    request.close();
    second.close();
    {
        let counts = counts.lock().unwrap();
        assert_eq!((counts.owner_drops, counts.value_drops, counts.finalizers), (0, 3, 2));
    }
    drop(request);
    assert_eq!(counts.lock().unwrap().owner_drops, 0);
    drop(second);
    assert_eq!(counts.lock().unwrap().owner_drops, 1);
    assert_eq!(counts.lock().unwrap().state_drops, 1);
    assert!(weak.upgrade().is_none());
}

#[test]
fn invalid_edge_is_checked_instead_of_reinterpreting_a_value() {
    struct Consumer;
    let calls = RcThreadSafety::new(Mutex::new(0usize));
    let inst_calls = calls.clone();
    let mut native = registry! {
        provide(App, instance(7u32)),
        provide(App, instance(String::from("foreign value"))),
        provide(App, move |_: Inject<u32>| {
            *inst_calls.lock().unwrap() += 1;
            Ok(Consumer)
        }),
    }
    .into_registry();
    let data = native.entries.get_mut(&TypeInfo::of::<Consumer>()).unwrap().sync_mut().unwrap();
    let RegistrationInstantiator::Linked(inst) = &mut data.instantiator else {
        panic!("expected a linked instantiator");
    };
    inst.keys = alloc::vec![TypeInfo::of::<String>()].into();
    data.metadata.dependencies = BTreeSet::from([Dependency {
        type_info: TypeInfo::of::<String>(),
    }]);
    let container = SyncContainer::new(native);

    let error = container.get::<Consumer>().err().unwrap();
    assert!(matches!(
        error,
        ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(error))
            if matches!(error.as_ref(), ResolveErrorKind::IncorrectType { expected, .. }
                if expected == &TypeInfo::of::<u32>())
    ));
    assert_eq!(*calls.lock().unwrap(), 0);
    assert_eq!(&**container.get::<String>().unwrap(), "foreign value");

    let id = container
        .inner
        .registry
        .indexed
        .iter()
        .position(|(key, _)| key == &TypeInfo::of::<String>())
        .unwrap();
    let edges = [RegistrationId(u32::try_from(id).unwrap())];
    assert!(matches!(
        InjectTransient::<u32>::resolve_linked(&container, &mut edges.iter()),
        Err(ResolveErrorKind::IncorrectType { expected, .. }) if expected == TypeInfo::of::<u32>()
    ));

    let edges = [];
    assert!(catch_unwind(AssertUnwindSafe(|| {
        Inject::<u32>::resolve_linked(&container, &mut edges.iter())
    }))
    .is_err());

    let edges = [RegistrationId(u32::MAX)];
    let panic = catch_unwind(AssertUnwindSafe(|| Inject::<u32>::resolve_linked(&container, &mut edges.iter())));
    assert!(panic.is_err());
    assert_eq!(*calls.lock().unwrap(), 0);
}

#[cfg(feature = "async")]
#[repr(align(256))]
struct BorrowingInst {
    counts: RcThreadSafety<Mutex<AdapterCounts>>,
    state: RcThreadSafety<AdapterState>,
    payload: String,
    original: bool,
}

#[cfg(feature = "async")]
impl Clone for BorrowingInst {
    fn clone(&self) -> Self {
        self.counts.lock().unwrap().clones += 1;
        Self {
            counts: self.counts.clone(),
            state: self.state.clone(),
            payload: self.payload.clone(),
            original: false,
        }
    }
}

#[cfg(feature = "async")]
impl Drop for BorrowingInst {
    fn drop(&mut self) {
        let mut counts = self.counts.lock().unwrap();
        if self.original {
            counts.owner_drops += 1;
        } else {
            counts.clone_drops += 1;
        }
    }
}

#[cfg(feature = "async")]
struct BorrowingCall<'a> {
    inst: &'a BorrowingInst,
}

#[cfg(feature = "async")]
impl Future for BorrowingCall<'_> {
    type Output = Result<u32, InstantiatorErrorKind<ResolveErrorKind, InstantiateErrorKind>>;

    fn poll(self: Pin<&mut Self>, _: &mut TaskContext<'_>) -> Poll<Self::Output> {
        assert!((self.inst as *const BorrowingInst).is_aligned());
        assert!(self.inst.original);
        assert_eq!(self.inst.payload, "original instantiator");
        let mut counts = self.inst.counts.lock().unwrap();
        assert_eq!(counts.owner_drops, 0);
        counts.polls += 1;
        if counts.ready {
            counts.calls += 1;
            Poll::Ready(Ok(41))
        } else {
            Poll::Pending
        }
    }
}

#[cfg(feature = "async")]
impl Drop for BorrowingCall<'_> {
    fn drop(&mut self) {
        assert!((self.inst as *const BorrowingInst).is_aligned());
        assert!(self.inst.original);
        assert_eq!(self.inst.payload, "original instantiator");
        let mut counts = self.inst.counts.lock().unwrap();
        assert_eq!(counts.owner_drops, 0);
        counts.future_drops += 1;
    }
}

#[cfg(feature = "async")]
impl AsyncInstantiator<()> for BorrowingInst {
    type Provides = u32;
    type Error = InstantiateErrorKind;

    async fn instantiate(&mut self, _: ()) -> Result<Self::Provides, Self::Error> {
        panic!("the custom linked hook must be used");
    }

    fn dependencies() -> BTreeSet<Dependency> {
        BTreeSet::new()
    }

    fn instantiate_linked(
        &self,
        _: &AsyncContainer,
        _: &[RegistrationId],
    ) -> impl Future<Output = Result<Self::Provides, InstantiatorErrorKind<ResolveErrorKind, InstantiateErrorKind>>> + SendSafety {
        self.counts.lock().unwrap().futures += 1;
        BorrowingCall { inst: self }
    }
}

#[cfg(feature = "async")]
#[tokio::test(flavor = "current_thread")]
async fn borrowing_inst_lives_until_pending_call_finishes_or_is_cancelled() {
    for (poll, complete) in [(false, false), (true, false), (true, true)] {
        let counts = RcThreadSafety::new(Mutex::new(AdapterCounts::default()));
        let state = RcThreadSafety::new(AdapterState(counts.clone()));
        let weak = RcThreadSafety::downgrade(&state);
        let finalized = counts.clone();
        let app = AsyncContainer::new(registry! {
            provide(Request, BorrowingInst {
                counts: counts.clone(),
                state,
                payload: "original instantiator".into(),
                original: true,
            }, finalizer = move |value: RcThreadSafety<u32>| {
                let finalized = finalized.clone();
                async move {
                    assert_eq!(*value, 41);
                    finalized.lock().unwrap().finalizers += 1;
                }
            }),
        });
        let request = app.clone().enter_build().unwrap();
        drop(app);
        let task_container = request.clone();
        let mut call = Box::pin(async move {
            let value = task_container.get::<u32>().await?;
            assert_eq!(*value, 41);
            assert!(RcThreadSafety::ptr_eq(&value, &task_container.get::<u32>().await.unwrap()));
            task_container.close().await;
            Ok::<_, ResolveErrorKind>(value)
        });
        drop(request);
        assert!(weak.upgrade().is_some());

        if poll {
            let mut cx = TaskContext::from_waker(Waker::noop());
            assert!(matches!(call.as_mut().poll(&mut cx), Poll::Pending));
            assert!(weak.upgrade().is_some());
            let counts = counts.lock().unwrap();
            assert_eq!((counts.futures, counts.polls, counts.future_drops, counts.calls), (1, 1, 0, 0));
            assert_eq!((counts.clones, counts.owner_drops), (0, 0));
        }

        if complete {
            counts.lock().unwrap().ready = true;
            let value = call.await.unwrap();
            assert_eq!(*value, 41);
        } else {
            drop(call);
        }
        assert!(weak.upgrade().is_none());
        let counts = counts.lock().unwrap();
        assert_eq!(
            (counts.clones, counts.clone_drops, counts.owner_drops, counts.state_drops),
            (0, 0, 1, 1)
        );
        assert_eq!(counts.futures, usize::from(poll));
        assert_eq!(counts.future_drops, usize::from(poll));
        assert_eq!(counts.calls, usize::from(complete));
        assert_eq!(counts.finalizers, usize::from(complete));
    }
}

#[test]
fn zero_sized_instantiator_is_owned_after_registry_erasure() {
    use core::sync::atomic::{AtomicUsize, Ordering};

    static CALLS: AtomicUsize = AtomicUsize::new(0);
    static CLONES: AtomicUsize = AtomicUsize::new(0);
    static DROPS: AtomicUsize = AtomicUsize::new(0);

    struct ZstInst;
    struct ZstValue;

    impl Clone for ZstInst {
        fn clone(&self) -> Self {
            CLONES.fetch_add(1, Ordering::SeqCst);
            Self
        }
    }

    impl Drop for ZstInst {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl Instantiator<()> for ZstInst {
        type Provides = ZstValue;
        type Error = InstantiateErrorKind;

        fn instantiate(&mut self, _: ()) -> Result<Self::Provides, Self::Error> {
            CALLS.fetch_add(1, Ordering::SeqCst);
            Ok(ZstValue)
        }

        fn dependencies() -> BTreeSet<Dependency> {
            BTreeSet::new()
        }
    }

    let native = registry! { provide(App, ZstInst) }.into_registry();
    let copied = native.clone();
    let first = SyncContainer::new(native);
    let cached = first.get::<ZstValue>().unwrap();
    assert!(RcThreadSafety::ptr_eq(&cached, &first.get::<ZstValue>().unwrap()));
    first.get_transient::<ZstValue>().unwrap();
    drop(first);
    assert_eq!(DROPS.load(Ordering::SeqCst), 2);
    let second = SyncContainer::new(copied);
    second.get_transient::<ZstValue>().unwrap();
    drop(second);

    assert_eq!(CALLS.load(Ordering::SeqCst), 3);
    assert_eq!(CLONES.load(Ordering::SeqCst), 3);
    assert_eq!(DROPS.load(Ordering::SeqCst), 4);
}

#[test]
fn errors_and_panics_preserve_indexed_dependencies_and_finalizer_ownership() {
    struct DependencyValue;
    struct Recovered;
    let events = RcThreadSafety::new(Mutex::new(Vec::new()));
    let dependency_events = events.clone();
    let dependency_finalizers = events.clone();
    let inst_events = events.clone();
    let finalizers = events.clone();
    let attempts = RcThreadSafety::new(Mutex::new(0usize));
    let inst_attempts = attempts.clone();
    let app = SyncContainer::new(registry! {
        provide(App, move || {
            dependency_events.lock().unwrap().push("dependency");
            Ok(DependencyValue)
        }, finalizer = move |_: RcThreadSafety<DependencyValue>| {
            dependency_finalizers.lock().unwrap().push("dependency finalized");
        }),
        provide(Request, move |_: Inject<DependencyValue>| {
            let attempt = {
                let mut attempts = inst_attempts.lock().unwrap();
                *attempts += 1;
                *attempts
            };
            inst_events.lock().unwrap().push("attempt");
            match attempt {
                1 => Err(InstantiateErrorKind::Custom(anyhow::anyhow!("construction failed"))),
                2 => panic!("construction panicked"),
                _ => Ok(Recovered),
            }
        }, finalizer = move |_: RcThreadSafety<Recovered>| finalizers.lock().unwrap().push("recovered finalized")),
    });
    let request = app.clone().enter_build().unwrap();

    assert_eq!(request.get::<Recovered>().err().unwrap().to_string(), "construction failed");
    assert!(catch_unwind(AssertUnwindSafe(|| request.get::<Recovered>())).is_err());
    drop(request.get::<Recovered>().unwrap());
    assert_eq!(*attempts.lock().unwrap(), 3);
    request.close();
    assert_eq!(
        *events.lock().unwrap(),
        ["dependency", "attempt", "attempt", "attempt", "recovered finalized"]
    );
    app.close();
    assert_eq!(
        *events.lock().unwrap(),
        [
            "dependency",
            "attempt",
            "attempt",
            "attempt",
            "recovered finalized",
            "dependency finalized"
        ]
    );
}
