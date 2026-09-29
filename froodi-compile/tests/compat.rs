//! Side-by-side compatibility fixture: every scenario is written once and compiled against both
//! the current engine (`froodi`) and the experimental one (`froodi_compile`). A scenario that
//! stops compiling or behaving the same on one side is API or semantic drift.

#![allow(clippy::unnecessary_wraps, reason = "factories return Result by contract")]

macro_rules! scenario {
    ($engine:ident) => {
        mod $engine {
            use $engine::{instance, registry, Container, DefaultScope::*, Inject, InstantiateErrorKind};

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
