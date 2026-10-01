extern crate std;

use alloc::{format, string::ToString as _, vec, vec::Vec};

use crate::compiled_registry as registry;
use crate::{
    instance, Config as RegistrationConfig, Container, Context, DefaultScope::*, Inject, InjectTransient, InstantiateErrorKind,
    ResolveErrorKind, ScopeWithErrorKind,
};
use anyhow::anyhow;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Barrier, Mutex,
    },
    thread,
    time::Duration,
};

#[derive(Clone)]
struct Config {
    url: &'static str,
}
struct Database {
    url: &'static str,
}

fn make_config() -> Result<Config, InstantiateErrorKind> {
    Ok(Config {
        url: "postgres://localhost",
    })
}

fn make_database(Inject(config): Inject<Config>) -> Result<Database, InstantiateErrorKind> {
    Ok(Database { url: config.url })
}

#[test]
fn resolves_an_inline_closure() {
    let container = Container::new(registry! {
        scope(App) [
            provide(make_config),
            provide(|Inject(config): Inject<Config>| Ok::<_, InstantiateErrorKind>(Database { url: config.url })),
        ],
    });

    assert_eq!(container.get::<Database>().unwrap().url, "postgres://localhost");
}

#[test]
fn resolves_a_captured_closure() {
    let url: &'static str = "postgres://captured";
    let container = Container::new(registry! {
        scope(App) [
            provide(move || Ok::<_, InstantiateErrorKind>(Config { url })),
            provide(make_database),
        ],
    });

    assert_eq!(container.get::<Database>().unwrap().url, "postgres://captured");
}

#[test]
fn resolves_an_instance() {
    let container = Container::new(registry! {
        scope(App) [
            provide(instance(Config { url: "postgres://instance" })),
            provide(make_database),
        ],
    });

    assert_eq!(container.get::<Database>().unwrap().url, "postgres://instance");
}

#[test]
fn accepts_provide_with_scope() {
    let container = Container::new(registry! {
        provide(App, make_config),
        scope(App) [
            provide(make_database),
        ],
    });

    assert_eq!(container.get::<Database>().unwrap().url, "postgres://localhost");
}

#[test]
fn resolves_across_extended_registries() {
    let configuration = registry! {
        scope(App) [
            provide(make_config),
        ],
    };
    let container = Container::new(registry! {
        scope(App) [
            provide(make_database),
        ],
        extend(configuration),
    });

    assert_eq!(container.get::<Database>().unwrap().url, "postgres://localhost");
}

#[test]
fn resolves_a_registry_made_only_of_extensions() {
    let container = Container::new(registry! {
        extend(
            registry! { scope(App) [ provide(make_config) ] },
            registry! { scope(App) [ provide(make_database) ] },
        ),
    });

    assert_eq!(container.get::<Database>().unwrap().url, "postgres://localhost");
}

#[test]
fn extends_an_empty_registry() {
    let container = Container::new(registry! {
        scope(App) [
            provide(make_config),
            provide(make_database),
        ],
        extend(registry!()),
    });

    assert_eq!(container.get::<Database>().unwrap().url, "postgres://localhost");
}

fn counted<T>(calls: &Arc<AtomicUsize>, value: T) -> impl FnMut() -> Result<T, InstantiateErrorKind> + Clone + Send + Sync + 'static
where
    T: Clone + Send + Sync + 'static,
{
    let calls = calls.clone();
    move || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(value.clone())
    }
}

fn calls() -> Arc<AtomicUsize> {
    Arc::new(AtomicUsize::new(0))
}

fn count(calls: &Arc<AtomicUsize>) -> usize {
    calls.load(Ordering::SeqCst)
}

#[test]
fn get_caches_the_provided_value() {
    let config_calls = calls();
    let container = Container::new(registry! {
        scope(App) [
            provide(counted(&config_calls, Config { url: "postgres://localhost" })),
        ],
    });

    let first = container.get::<Config>().unwrap();
    let second = container.get::<Config>().unwrap();

    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(count(&config_calls), 1);
}

#[test]
fn inject_shares_the_cached_value() {
    struct Pool(#[allow(dead_code)] Config);
    struct Repository(Arc<Config>);
    let config_calls = calls();
    let container = Container::new(registry! {
        scope(App) [
            provide(counted(&config_calls, Config { url: "postgres://localhost" })),
            provide(|Inject(config): Inject<Config>| Ok::<_, InstantiateErrorKind>(Repository(config))),
            provide(|Inject(config): Inject<Config>| Ok::<_, InstantiateErrorKind>(Pool(Config { url: config.url }))),
        ],
    });

    let repository = container.get::<Repository>().unwrap();
    let _pool = container.get::<Pool>().unwrap();
    let config = container.get::<Config>().unwrap();

    assert!(Arc::ptr_eq(&repository.0, &config));
    assert_eq!(count(&config_calls), 1);
}

#[test]
fn get_transient_constructs_every_time_and_skips_the_cache() {
    let config_calls = calls();
    let container = Container::new(registry! {
        scope(App) [
            provide(counted(&config_calls, Config { url: "postgres://localhost" })),
        ],
    });

    let first: Config = container.get_transient::<Config>().unwrap();
    let _second: Config = container.get_transient::<Config>().unwrap();
    assert_eq!(first.url, "postgres://localhost");
    assert_eq!(count(&config_calls), 2);

    let _provided = container.get::<Config>().unwrap();
    assert_eq!(count(&config_calls), 3);
}

#[test]
fn inject_transient_builds_a_fresh_value_for_each_dependent() {
    struct First(Config);
    struct Second(#[allow(dead_code)] Config);
    let config_calls = calls();
    let container = Container::new(registry! {
        scope(App) [
            provide(counted(&config_calls, Config { url: "postgres://localhost" })),
            provide(|InjectTransient(config): InjectTransient<Config>| Ok::<_, InstantiateErrorKind>(First(config))),
            provide(|InjectTransient(config): InjectTransient<Config>| Ok::<_, InstantiateErrorKind>(Second(config))),
        ],
    });

    assert_eq!(container.get::<First>().unwrap().0.url, "postgres://localhost");
    let _second = container.get::<Second>().unwrap();
    assert_eq!(count(&config_calls), 2);
    let _provided = container.get::<Config>().unwrap();
    assert_eq!(count(&config_calls), 3);
}

#[test]
fn get_without_cache_provides_constructs_every_time() {
    let config_calls = calls();
    let container = Container::new(registry! {
        scope(App) [
            provide(
                counted(&config_calls, Config { url: "postgres://localhost" }),
                config = RegistrationConfig { cache_provides: false },
            ),
        ],
    });

    let first = container.get::<Config>().unwrap();
    let second = container.get::<Config>().unwrap();

    assert!(!Arc::ptr_eq(&first, &second));
    assert_eq!(count(&config_calls), 2);
}

#[test]
fn a_narrower_scope_is_not_accessible_from_a_wider_container() {
    let container = Container::new(registry! {
        provide(Request, || Ok::<_, InstantiateErrorKind>(Config { url: "request" })),
    });

    let err = container.get::<Config>().err().expect("request scope is not reachable from app");

    match err {
        ResolveErrorKind::NoAccessible {
            expected_scope_data,
            actual_scope_data,
        } => {
            assert_eq!(expected_scope_data.name, "request");
            assert_eq!(actual_scope_data.name, "app");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn child_containers_share_wider_values_and_own_their_scope() {
    #[derive(Clone)]
    struct Handler;
    let config_calls = calls();
    let handler_calls = calls();
    let app = Container::new(registry! {
        provide(Runtime, || Ok::<_, InstantiateErrorKind>(Database { url: "runtime" })),
        provide(App, counted(&config_calls, Config { url: "app" })),
        provide(Request, counted(&handler_calls, Handler)),
    });

    let first = app.clone().enter_build().unwrap();
    let second = app.clone().enter_build().unwrap();

    assert!(Arc::ptr_eq(&first.get::<Config>().unwrap(), &second.get::<Config>().unwrap()));
    assert!(!Arc::ptr_eq(&first.get::<Handler>().unwrap(), &second.get::<Handler>().unwrap()));
    assert!(Arc::ptr_eq(&first.get::<Handler>().unwrap(), &first.get::<Handler>().unwrap()));
    assert_eq!(first.get::<Database>().unwrap().url, "runtime");
    assert_eq!((count(&config_calls), count(&handler_calls)), (1, 2));
}

#[test]
fn with_scope_creates_the_intermediate_scopes() {
    #[derive(Clone)]
    struct Handler;
    let app = Container::new(registry! {
        provide(Request, || Ok::<_, InstantiateErrorKind>(Handler)),
        provide(Action, |Inject(_handler): Inject<Handler>| Ok::<_, InstantiateErrorKind>(Config { url: "action" })),
    });

    let action = app.clone().enter().with_scope(Action).build().unwrap();
    assert_eq!(action.get::<Config>().unwrap().url, "action");

    match app.enter().with_scope(App).build() {
        Err(ScopeWithErrorKind::NoChildRegistriesWithScope { name, .. }) => assert_eq!(name, "app"),
        Err(other) => panic!("unexpected error: {other}"),
        Ok(_) => panic!("app is not below app"),
    }
}

#[test]
fn new_with_start_scope_starts_in_a_skipped_scope() {
    let runtime = Container::new_compiled_with_start_scope(
        registry! {
            provide(Runtime, || Ok::<_, InstantiateErrorKind>(Config { url: "runtime" })),
            provide(App, || Ok::<_, InstantiateErrorKind>(Database { url: "app" })),
        },
        Runtime,
    );

    assert_eq!(runtime.get::<Config>().unwrap().url, "runtime");
    assert!(runtime.get::<Database>().is_err());
    assert_eq!(runtime.enter_build().unwrap().get::<Database>().unwrap().url, "app");
}

#[test]
fn get_returns_a_context_value_nothing_registers() {
    struct RequestId(u64);
    let app = Container::new(registry! {
        scope(App) [ provide(make_config) ],
    });
    let mut context = Context::new();
    context.insert(RequestId(7));

    let request = app.enter().with_context(context).build().unwrap();

    assert_eq!(request.get::<RequestId>().unwrap().0, 7);
    assert!(request.get_transient::<RequestId>().is_err());
}

#[test]
fn a_context_value_overrides_the_factory_for_dependents_and_descendants() {
    let config_calls = calls();
    let app = Container::new(registry! {
        provide(App, counted(&config_calls, Config { url: "factory" })),
        provide(Request, make_database),
        provide(Action, |Inject(config): Inject<Config>| Ok::<_, InstantiateErrorKind>(Database { url: config.url }).map(|db| (db.url, 0_u8))),
    });
    let mut context = Context::new();
    context.insert(Config { url: "context" });

    let request = app.enter().with_context(context).build().unwrap();
    let action = request.clone().enter_build().unwrap();

    assert_eq!(request.get::<Database>().unwrap().url, "context");
    assert_eq!(action.get::<(&'static str, u8)>().unwrap().0, "context");
    assert_eq!(request.get_transient::<Config>().unwrap().url, "factory");
    assert_eq!(count(&config_calls), 1);
}

fn events() -> Arc<Mutex<Vec<&'static str>>> {
    Arc::new(Mutex::new(Vec::new()))
}

fn record<T>(events: &Arc<Mutex<Vec<&'static str>>>, name: &'static str) -> impl FnMut(Arc<T>) + Clone + Send + Sync + 'static {
    let events = events.clone();
    move |_| events.lock().unwrap().push(name)
}

#[test]
fn close_runs_finalizers_in_reverse_order_once_and_clears_the_cache() {
    let log = events();
    let config_calls = calls();
    let container = Container::new(registry! {
        scope(App) [
            provide(counted(&config_calls, Config { url: "postgres://localhost" }), finalizer = record(&log, "config")),
            provide(make_database, finalizer = record(&log, "database")),
        ],
    });

    let _database = container.get::<Database>().unwrap();
    container.close();
    container.close();
    assert_eq!(*log.lock().unwrap(), vec!["database", "config"]);

    let _config = container.get::<Config>().unwrap();
    assert_eq!(count(&config_calls), 2);
}

#[test]
fn dropping_the_last_handle_closes_the_container() {
    let log = events();
    let app = Container::new(registry! {
        provide(App, make_config, finalizer = record(&log, "config")),
        provide(Request, make_database, finalizer = record(&log, "database")),
    });

    let request = app.clone().enter_build().unwrap();
    let _database = request.get::<Database>().unwrap();
    drop(request);
    assert_eq!(*log.lock().unwrap(), vec!["database"]);

    drop(app);
    assert_eq!(*log.lock().unwrap(), vec!["database", "config"]);
}

#[test]
fn the_container_itself_is_a_root_scope_dependency() {
    struct Probe(bool);
    let app = Container::new(registry! {
        provide(App, make_config),
        provide(Request, |Inject(container): Inject<Container>| {
            Ok::<_, InstantiateErrorKind>(Probe(container.get::<Config>().is_ok()))
        }),
    });
    let request = app.enter_build().unwrap();

    let root = request.get::<Container>().unwrap();

    assert!(!request.get::<Probe>().unwrap().0);
    assert!(root.get::<Config>().is_err());
}

#[test]
fn concurrent_get_constructs_a_cached_value_once() {
    let config_calls = calls();
    let slow = {
        let config_calls = config_calls.clone();
        move || {
            config_calls.fetch_add(1, Ordering::SeqCst);
            thread::sleep(Duration::from_millis(20));
            Ok::<_, InstantiateErrorKind>(Config { url: "slow" })
        }
    };
    let container = Container::new(registry! {
        scope(App) [ provide(slow) ],
    });
    let barrier = Arc::new(Barrier::new(8));

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let (container, barrier) = (container.clone(), barrier.clone());
            thread::spawn(move || {
                barrier.wait();
                container.get::<Config>().unwrap()
            })
        })
        .collect();
    let values: Vec<_> = handles.into_iter().map(|handle| handle.join().unwrap()).collect();

    assert_eq!(count(&config_calls), 1);
    assert!(values.iter().all(|value| Arc::ptr_eq(value, &values[0])));
}

#[test]
fn errors_keep_froodis_nesting() {
    let container = Container::new(registry! {
        scope(App) [
            provide(|| Err::<Config, _>(InstantiateErrorKind::Custom(anyhow!("no config")))),
            provide(make_database),
        ],
    });

    // Froodi does not export `InstantiatorErrorKind`, so the shape is compared by `Debug`.
    let factory = format!("{:?}", container.get::<Config>().err().unwrap());
    assert!(factory.starts_with("Instantiator(Factory(Custom("), "{factory}");
    assert_eq!(container.get::<Config>().err().unwrap().to_string(), "no config");
    let deps = format!("{:?}", container.get::<Database>().err().unwrap());
    assert!(deps.starts_with("Instantiator(Deps(Instantiator(Factory(Custom("), "{deps}");
    assert!(matches!(container.get::<u8>(), Err(ResolveErrorKind::NoInstantiator { .. })));
}

#[test]
fn a_failed_construction_still_finalizes_the_dependencies_it_built() {
    let log = events();
    let container = Container::new(registry! {
        scope(App) [
            provide(make_config, finalizer = record(&log, "config")),
            provide(
                |Inject(_config): Inject<Config>| Err::<Database, _>(InstantiateErrorKind::Custom(anyhow!("down"))),
                finalizer = record(&log, "database"),
            ),
        ],
    });

    assert!(container.get::<Database>().is_err());
    container.close();

    assert_eq!(*log.lock().unwrap(), vec!["config"]);
}

#[test]
fn closing_a_scope_finalizes_only_its_own_values() {
    let log = events();
    let app = Container::new(registry! {
        provide(App, make_config, finalizer = record(&log, "config")),
        provide(Request, make_database, finalizer = record(&log, "database")),
    });
    let request = app.clone().enter_build().unwrap();

    let _database = request.get::<Database>().unwrap();
    request.close();
    assert_eq!(*log.lock().unwrap(), vec!["database"]);

    app.close();
    assert_eq!(*log.lock().unwrap(), vec!["database", "config"]);
}

mod custom_scope {
    use crate::{registry as dynamic_registry, Container, InstantiateErrorKind, ResolveErrorKind, Scope, ScopeData, Scopes};

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

    struct Wide;
    struct Narrow;

    #[test]
    fn resolves_through_a_user_defined_scope_hierarchy() {
        let container = Container::new(dynamic_registry! {
            scope(MyScope::Work) [ provide(|| Ok::<_, InstantiateErrorKind>(Wide)) ],
            provide(MyScope::Task, || Ok::<_, InstantiateErrorKind>(Narrow)),
        });

        assert!(container.get::<Wide>().is_ok());
        match container.get::<Narrow>() {
            Err(ResolveErrorKind::NoAccessible {
                expected_scope_data,
                actual_scope_data,
            }) => {
                assert_eq!((expected_scope_data.name, actual_scope_data.name), ("task", "work"));
            }
            other => panic!("expected NoAccessible, got {:?}", other.err()),
        }
        let task = container.enter().with_scope(MyScope::Task).build().unwrap();
        assert!(task.get::<Narrow>().is_ok());
    }
}

#[test]
fn get_transient_respects_scopes_and_builds_in_the_owning_container() {
    let app = Container::new(registry! {
        provide(App, make_config),
        provide(App, make_database),
        provide(Request, || Ok::<_, InstantiateErrorKind>((0_u8, 0_u16))),
    });
    assert!(matches!(
        app.get_transient::<(u8, u16)>(),
        Err(ResolveErrorKind::NoAccessible { .. })
    ));

    // A request-level context value of Config does not reach an App-owned transient build.
    let mut context = Context::new();
    context.insert(Config { url: "context" });
    let request = app.enter().with_context(context).build().unwrap();
    assert_eq!(request.get_transient::<Database>().unwrap().url, "postgres://localhost");
    assert_eq!(request.get::<Database>().unwrap().url, "postgres://localhost");
}

#[test]
fn resolves_a_named_factory_chain() {
    let container = Container::new(registry! {
        scope(App) [
            provide(make_config),
            provide(make_database),
        ],
    });

    let database = container.get::<Database>().unwrap();

    assert_eq!(database.url, "postgres://localhost");
}

#[cfg(feature = "async")]
mod async_impl {
    extern crate std;

    use crate::compiled_async_registry as async_registry;
    use crate::compiled_registry as registry;
    use alloc::{boxed::Box, vec, vec::Vec};
    use core::{future::Future, pin::Pin};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };

    use crate::{async_impl::Container, DefaultScope::*, Inject, InjectTransient, InstantiateErrorKind};

    #[derive(Clone)]
    struct Config(&'static str);
    struct Database(Arc<Config>);
    struct Snapshot(Config);

    async fn make_config() -> Result<Config, InstantiateErrorKind> {
        Ok(Config("async"))
    }

    async fn make_database(Inject(config): Inject<Config>) -> Result<Database, InstantiateErrorKind> {
        Ok(Database(config))
    }

    #[tokio::test]
    async fn resolves_and_caches_an_async_factory_chain() {
        let container = Container::new(async_registry! {
            scope(App) [
                provide(make_config),
                provide(make_database),
            ],
        });

        let database = container.get::<Database>().await.unwrap();

        assert_eq!(database.0 .0, "async");
        assert!(Arc::ptr_eq(&database.0, &container.get::<Config>().await.unwrap()));
    }

    #[tokio::test]
    async fn an_async_factory_depends_on_a_sync_registration() {
        let container = Container::new(async_registry! {
            scope(App) [
                provide(make_database),
                provide(async |InjectTransient(config): InjectTransient<Config>| Ok::<_, InstantiateErrorKind>(Snapshot(config))),
            ],
            extend(registry! {
                scope(App) [ provide(|| Ok::<_, InstantiateErrorKind>(Config("sync"))) ],
            }),
        });

        assert_eq!(container.get::<Database>().await.unwrap().0 .0, "sync");
        assert_eq!(container.get::<Snapshot>().await.unwrap().0 .0, "sync");
        assert_eq!(container.get_transient::<Config>().await.unwrap().0, "sync");
    }

    #[tokio::test]
    async fn close_awaits_async_finalizers_in_reverse_order() {
        fn recorder<T>(
            log: &Arc<Mutex<Vec<&'static str>>>,
            name: &'static str,
        ) -> impl FnMut(Arc<T>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Clone + Send + Sync + 'static {
            let log = log.clone();
            move |_| {
                let log = log.clone();
                Box::pin(async move { log.lock().unwrap().push(name) })
            }
        }
        let log = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = {
            let calls = calls.clone();
            move || {
                let calls = calls.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, InstantiateErrorKind>(Config("async"))
                }
            }
        };
        let container = Container::new(async_registry! {
            scope(App) [
                provide(counted, finalizer = recorder::<Config>(&log, "config")),
                provide(make_database, finalizer = recorder::<Database>(&log, "database")),
            ],
        });

        let _database = container.get::<Database>().await.unwrap();
        container.close().await;

        assert_eq!(*log.lock().unwrap(), vec!["database", "config"]);
        let _config = container.get::<Config>().await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn request_scopes_work_in_async_containers() {
        struct Handler;
        let app = Container::new(async_registry! {
            provide(App, make_config),
            provide(Request, async |Inject(_config): Inject<Config>| Ok::<_, InstantiateErrorKind>(Handler)),
        });

        assert!(app.get::<Handler>().await.is_err());
        let request = app.clone().enter_build().unwrap();
        assert!(request.get::<Handler>().await.is_ok());
    }
}
