//! Side-by-side compatibility fixture: every scenario is written once and compiled against both
//! the current engine (`froodi`) and the experimental one (`froodi_compile`). A scenario that
//! stops compiling or behaving the same on one side is API or semantic drift.

#![allow(clippy::unnecessary_wraps, reason = "factories return Result by contract")]

macro_rules! scenario {
    ($engine:ident) => {
        mod $engine {
            use std::sync::Arc;
            use $engine::{instance, registry, Container, DefaultScope::*, Inject, InjectTransient, InstantiateErrorKind, ResolveErrorKind};

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

            fn counted<T>(
                calls: &std::sync::Arc<std::sync::atomic::AtomicUsize>,
                value: T,
            ) -> impl FnMut() -> Result<T, InstantiateErrorKind> + Clone + Send + Sync + 'static
            where
                T: Clone + Send + Sync + 'static,
            {
                let calls = calls.clone();
                move || {
                    calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(value.clone())
                }
            }

            fn calls() -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
                std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0))
            }

            fn count(calls: &std::sync::Arc<std::sync::atomic::AtomicUsize>) -> usize {
                calls.load(std::sync::atomic::Ordering::SeqCst)
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

                assert!(std::sync::Arc::ptr_eq(&first, &second));
                assert_eq!(count(&config_calls), 1);
            }

            #[test]
            fn inject_shares_the_cached_value() {
                struct Pool(Config);
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

                let _shared = container.get::<Config>().unwrap();
                assert_eq!(count(&config_calls), 3);
            }

            #[test]
            fn inject_transient_builds_a_fresh_value_for_each_dependent() {
                struct First(Config);
                struct Second(Config);
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
                let _shared = container.get::<Config>().unwrap();
                assert_eq!(count(&config_calls), 3);
            }

            #[test]
            fn get_without_cache_provides_constructs_every_time() {
                let config_calls = calls();
                let container = Container::new(registry! {
                    scope(App) [
                        provide(
                            counted(&config_calls, Config { url: "postgres://localhost" }),
                            config = $engine::Config { cache_provides: false },
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
                    Err($engine::ScopeWithErrorKind::NoChildRegistriesWithScope { name, .. }) => assert_eq!(name, "app"),
                    Err(other) => panic!("unexpected error: {other}"),
                    Ok(_) => panic!("app is not below app"),
                }
            }

            #[test]
            fn new_with_start_scope_starts_in_a_skipped_scope() {
                let runtime = Container::new_with_start_scope(
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
                let mut context = $engine::Context::new();
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
                let mut context = $engine::Context::new();
                context.insert(Config { url: "context" });

                let request = app.enter().with_context(context).build().unwrap();
                let action = request.clone().enter_build().unwrap();

                assert_eq!(request.get::<Database>().unwrap().url, "context");
                assert_eq!(action.get::<(&'static str, u8)>().unwrap().0, "context");
                assert_eq!(request.get_transient::<Config>().unwrap().url, "factory");
                assert_eq!(count(&config_calls), 1);
            }

            fn events() -> Arc<std::sync::Mutex<Vec<&'static str>>> {
                Arc::new(std::sync::Mutex::new(Vec::new()))
            }

            fn record<T>(events: &Arc<std::sync::Mutex<Vec<&'static str>>>, name: &'static str) -> impl FnMut(Arc<T>) + Clone + Send + Sync + 'static {
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

                assert!(request.get::<Probe>().unwrap().0 == false);
                assert!(root.get::<Config>().is_err());
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
        }
    };
}

scenario!(froodi);
scenario!(froodi_compile);
