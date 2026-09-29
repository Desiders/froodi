//! Side-by-side compatibility fixture: every scenario is written once and compiled against both
//! the current engine (`froodi`) and the experimental one (`froodi_compile`). A scenario that
//! stops compiling or behaving the same on one side is API or semantic drift.

macro_rules! scenario {
    ($engine:ident) => {
        mod $engine {
            use $engine::{registry, Container, DefaultScope::*, Inject, InstantiateErrorKind};

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
