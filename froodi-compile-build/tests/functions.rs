//! Named factories: the signature of a function declared in the same file is the source of truth.

use froodi_compile_build::{analyze, Dependency, Mode, Outcome, Reason};

fn outcomes(source: &str) -> Vec<Outcome> {
    analyze(source)
        .unwrap()
        .registrations()
        .into_iter()
        .map(|registration| registration.outcome.clone())
        .collect()
}

fn inject(ty: &str) -> Dependency {
    Dependency {
        ty: ty.to_owned(),
        mode: Mode::Inject,
    }
}

fn inject_transient(ty: &str) -> Dependency {
    Dependency {
        ty: ty.to_owned(),
        mode: Mode::InjectTransient,
    }
}

fn resolved(provides: &str, deps: Vec<Dependency>) -> Outcome {
    Outcome::Resolved {
        provides: provides.to_owned(),
        deps,
    }
}

#[test]
fn reads_the_signature_of_a_function_in_the_same_module() {
    let source = r"
        fn make_database(Inject(config): Inject<Config>) -> InstantiatorResult<Database> {
            Ok(Database::connect(&config.url))
        }

        fn make_handler(
            Inject(repo): Inject<UserRepository>,
            InjectTransient(clock): InjectTransient<Clock>,
        ) -> Result<Handler, InstantiateErrorKind> {
            Ok(Handler { repo, clock })
        }

        fn build() -> Registry {
            registry! {
                scope(Request) [
                    provide(make_database),
                    provide(make_handler, config = Config { cache_provides: false }),
                ],
            }
        }
    ";
    assert_eq!(
        outcomes(source),
        [
            resolved("Database", vec![inject("Config")]),
            resolved("Handler", vec![inject("UserRepository"), inject_transient("Clock")]),
        ]
    );
}

#[test]
fn reads_async_functions_and_functions_declared_after_use() {
    let source = r"
        async fn build() -> AsyncRegistry {
            async_registry! {
                provide(App, self::connect_pool),
            }
        }

        async fn connect_pool(Inject(config): Inject<Settings>) -> InstantiatorResult<Arc<Pool>> {
            Pool::connect(&config.url).await
        }
    ";
    assert_eq!(outcomes(source), [resolved("Arc<Pool>", vec![inject("Settings")])]);
}

#[test]
fn reads_functions_declared_in_the_enclosing_block_and_inline_module() {
    let source = r"
        mod wiring {
            fn make_cache() -> InstantiatorResult<Cache> { Ok(Cache::default()) }

            pub fn build() -> Registry {
                fn make_metrics(Inject(cache): Inject<Cache>) -> InstantiatorResult<Metrics> {
                    Ok(Metrics::new(cache))
                }
                registry! {
                    provide(App, make_cache),
                    provide(App, make_metrics),
                }
            }
        }
    ";
    assert_eq!(
        outcomes(source),
        [resolved("Cache", vec![]), resolved("Metrics", vec![inject("Cache")])]
    );
}

#[test]
fn a_function_of_another_module_or_crate_is_not_in_the_file() {
    let source = r"
        use crate::db::make_database;

        fn make_local() -> InstantiatorResult<Local> { Ok(Local) }

        mod nested {
            fn build() -> Registry {
                registry! {
                    provide(App, make_database),
                    provide(App, crate::db::make_pool),
                    provide(App, Handler::new),
                    provide(App, make_local),
                }
            }
        }
    ";
    let paths: Vec<_> = outcomes(source)
        .into_iter()
        .map(|outcome| match outcome {
            Outcome::Unresolved(Reason::FactoryNotInFile { path }) => path,
            other => panic!("expected FactoryNotInFile, got {other:?}"),
        })
        .collect();
    assert_eq!(paths, ["make_database", "crate::db::make_pool", "Handler::new", "make_local"]);
}

#[test]
fn a_parameter_or_let_binding_shadows_a_function_of_the_same_name() {
    let source = r"
        fn make_repo(Inject(db): Inject<Database>) -> InstantiatorResult<Repo> { Ok(Repo(db)) }

        fn build(make_database: impl Fn() -> InstantiatorResult<Database>) -> Registry {
            let registry = registry! { provide(App, make_repo) };
            let make_repo = |Inject(db): Inject<Database>| Ok::<_, InstantiateErrorKind>(Repo(db));
            registry! {
                provide(App, make_database),
                provide(App, make_repo),
            }
        }
    ";
    assert_eq!(
        outcomes(source),
        [
            resolved("Repo", vec![inject("Database")]),
            Outcome::Unresolved(Reason::FactoryIsALocalValue {
                name: "make_database".to_owned()
            }),
            Outcome::Unresolved(Reason::FactoryIsALocalValue {
                name: "make_repo".to_owned()
            }),
        ]
    );
}

#[test]
fn a_generic_function_is_fixed_by_inference_at_the_call_site() {
    let source = r"
        fn make_repo<D: Driver>(Inject(driver): Inject<D>) -> InstantiatorResult<Repo<D>> { Ok(Repo::new(driver)) }
        fn make_client(Inject(http): Inject<impl Transport>) -> InstantiatorResult<Client> { todo!() }

        fn build() -> Registry {
            registry! {
                provide(App, make_repo::<Postgres>),
                provide(App, make_client),
            }
        }
    ";
    assert_eq!(
        outcomes(source),
        [
            Outcome::Unresolved(Reason::GenericFactory {
                name: "make_repo".to_owned()
            }),
            Outcome::Unresolved(Reason::GenericFactory {
                name: "make_client".to_owned()
            }),
        ]
    );
}

#[test]
fn parameters_and_return_types_outside_the_froodi_vocabulary_stay_unresolved() {
    let source = r"
        fn make_from_map(MapInject(value): MapInject<Update>) -> InstantiatorResult<Handler> { todo!() }
        fn make_plain(Inject(config): Inject<Config>) -> AppResult<Handler> { todo!() }

        fn build() -> Registry {
            registry! {
                provide(App, make_from_map),
                provide(App, make_plain),
            }
        }
    ";
    assert_eq!(
        outcomes(source),
        [
            Outcome::Unresolved(Reason::UnknownDependencyResolver {
                index: 0,
                ty: "MapInject<Update>".to_owned()
            }),
            Outcome::Unresolved(Reason::ReturnTypeIsNotAResult {
                ty: "AppResult<Handler>".to_owned()
            }),
        ]
    );
}
