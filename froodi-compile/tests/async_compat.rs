//! Async scenarios written once and run against both engines, like `compat.rs`.
#![cfg(all(feature = "async", feature = "thread_safe"))]
#![allow(clippy::unnecessary_wraps, reason = "factories return Result by contract")]

macro_rules! scenario {
    ($engine:ident) => {
        mod $engine {
            use std::sync::{
                atomic::{AtomicUsize, Ordering},
                Arc, Mutex,
            };

            use $engine::{
                async_impl::Container, async_registry, registry, DefaultScope::*, Inject, InjectTransient, InstantiateErrorKind,
            };

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
                ) -> impl FnMut(Arc<T>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Clone + Send + Sync + 'static
                {
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
    };
}

scenario!(froodi);
scenario!(froodi_compile);
