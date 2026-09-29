//! The same instantiator can occupy different slots in independently linked plans.

use froodi_compile::{
    instance, registry, Container, DefaultScope::App, DependencyResolver, Inject, InjectTransient, InstantiateErrorKind, ResolveErrorKind,
};

#[derive(Clone)]
struct First(usize);
#[derive(Clone)]
struct Second(usize);
struct Resolved;

impl DependencyResolver for Resolved {
    type Error = ResolveErrorKind;

    fn resolve(_: &Container) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

struct Combined([usize; 3]);

fn combine(
    Inject(first): Inject<First>,
    _: Resolved,
    InjectTransient(second): InjectTransient<Second>,
    Inject(again): Inject<First>,
) -> Result<Combined, InstantiateErrorKind> {
    Ok(Combined([first.0, second.0, again.0]))
}

#[test]
fn parameter_order_survives_resolvers_repeated_targets_and_different_topologies() {
    let first = Container::new(registry! {
        scope(App) [provide(instance(First(11))), provide(combine), provide(instance(Second(22)))],
    });
    let second = Container::new(registry! {
        scope(App) [provide(instance(Second(44))), provide(instance(First(33))), provide(combine)],
    });
    assert_eq!(first.get::<Combined>().unwrap().0, [11, 22, 11]);
    assert_eq!(second.get_transient::<Combined>().unwrap().0, [33, 44, 33]);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_parameter_order_survives_resolvers_and_repeated_targets() {
    let container = froodi_compile::async_impl::Container::new(froodi_compile::async_registry! {
        provide(App, async |Inject(first): Inject<First>, _: Resolved,
            InjectTransient(second): InjectTransient<Second>, Inject(again): Inject<First>| {
                Ok::<_, InstantiateErrorKind>(Combined([first.0, second.0, again.0]))
            }),
        extend(registry! {
            scope(App) [provide(instance(Second(22))), provide(instance(First(11)))],
        }),
    });
    assert_eq!(container.get::<Combined>().await.unwrap().0, [11, 22, 11]);
}

#[test]
fn nested_import_replacement_keeps_storage_type_and_finalizer_identity() {
    use froodi_compile::{runtime, thread_safety::RcThreadSafety, Config};
    use std::sync::Mutex;
    #[repr(align(64))]
    struct Aligned(String);
    struct Consumer(String);
    let log = RcThreadSafety::new(Mutex::new(Vec::new()));
    let original_log = log.clone();
    let replacement_log = log.clone();
    let original = registry! {
        provide(App, || Ok::<_, InstantiateErrorKind>(Aligned("original".into())),
            finalizer = move |_: RcThreadSafety<Aligned>| original_log.lock().unwrap().push("original")),
    }
    .into_runtime();
    let replacement = registry! {
        provide(App, || Ok::<_, InstantiateErrorKind>(Aligned("replacement".into())),
            config = Config { cache_provides: false },
            finalizer = move |_: RcThreadSafety<Aligned>| replacement_log.lock().unwrap().push("replacement")),
    }
    .into_runtime()
    .replacing();
    let nested = registry! { extend(original), extend(replacement) }.into_runtime();
    let container = Container::new(registry! {
        provide(App, runtime::<Aligned>(), config = Config { cache_provides: false }),
        provide(App, |_: Resolved, a: Inject<Aligned>, b: InjectTransient<Aligned>, again: Inject<Aligned>| {
            assert_eq!(a.0.0, b.0.0);
            assert_eq!(a.0.0, again.0.0);
            Ok::<_, InstantiateErrorKind>(Consumer(b.0.0))
        }),
        extend(nested),
    });
    let retained = container.clone();
    drop(container);
    assert_eq!(retained.get_transient::<Consumer>().unwrap().0, "replacement");
    retained.close();
    assert_eq!(*log.lock().unwrap(), ["replacement", "replacement"]);
    drop(retained);
    assert_eq!(log.lock().unwrap().len(), 2);
}
