//! Inline closures and `instance(...)`: the types are written only when the author spells them.

use froodi_compile_build::{analyze, Dependency, Mode, Outcome, Reason};

fn outcomes(registry: &str) -> Vec<Outcome> {
    let source = format!("fn build(config: Config) -> Registry {{ {registry} }}");
    analyze(&source)
        .unwrap()
        .registrations()
        .into_iter()
        .map(|registration| registration.outcome.clone())
        .collect()
}

fn resolved(provides: &str, deps: &[(&str, Mode)]) -> Outcome {
    Outcome::Resolved {
        provides: provides.to_owned(),
        deps: deps
            .iter()
            .map(|(ty, mode)| Dependency {
                ty: (*ty).to_owned(),
                mode: *mode,
            })
            .collect(),
    }
}

#[test]
fn a_closure_reads_its_annotated_parameters_but_its_return_type_is_usually_not_written() {
    let outcomes = outcomes(
        r"registry! {
            scope(Request) [
                provide(|Inject(db): Inject<Database>| Ok::<_, InstantiateErrorKind>(Repo(db))),
                provide(|Inject(db): Inject<Database>| Ok(Repo(db))),
            ],
        }",
    );
    assert_eq!(
        outcomes,
        [
            Outcome::Unresolved(Reason::ClosureReturnTypeNotWritten),
            Outcome::Unresolved(Reason::ClosureReturnTypeNotWritten),
        ]
    );
}

#[test]
fn a_closure_resolves_when_its_return_type_is_annotated_or_turbofished() {
    let outcomes = outcomes(
        r"registry! {
            scope(Request) [
                provide(|Inject(db): Inject<Database>| -> InstantiatorResult<Repo> { Ok(Repo(db)) }),
                provide(|InjectTransient(clock): InjectTransient<Clock>| Ok::<Stamp, InstantiateErrorKind>(clock.now())),
                provide(|| { let value = 1; Ok::<Counter, InstantiateErrorKind>(Counter(value)) }),
            ],
        }",
    );
    assert_eq!(
        outcomes,
        [
            resolved("Repo", &[("Database", Mode::Inject)]),
            resolved("Stamp", &[("Clock", Mode::InjectTransient)]),
            resolved("Counter", &[]),
        ]
    );
}

#[test]
fn an_async_closure_reads_the_turbofish_of_its_async_block() {
    let outcomes = outcomes(
        r"async_registry! {
            provide(App, |Inject(config): Inject<Config>| async move { Ok::<Pool, InstantiateErrorKind>(Pool::new(&config)) }),
            provide(App, |Inject(config): Inject<Config>| async move { Pool::connect(&config).await }),
        }",
    );
    assert_eq!(
        outcomes,
        [
            resolved("Pool", &[("Config", Mode::Inject)]),
            Outcome::Unresolved(Reason::ClosureReturnTypeNotWritten),
        ]
    );
}

#[test]
fn an_unannotated_closure_parameter_is_reported_by_index() {
    let outcomes = outcomes(
        r"registry! {
            provide(Request, |Inject(config): Inject<Config>, Inject(greeter)| -> InstantiatorResult<Handler> { todo!() }),
        }",
    );
    assert_eq!(outcomes, [Outcome::Unresolved(Reason::ClosureParameterNotAnnotated { index: 1 })]);
}

#[test]
fn an_instance_is_typed_only_by_a_struct_literal_or_a_turbofish() {
    let outcomes = outcomes(
        r"registry! {
            provide(App, instance(config)),
            provide(App, instance(Settings { verbose: true })),
            provide(App, instance::<Arc<Pool>>(pool.clone())),
            provide(App, froodi::instance(Config::default())),
        }",
    );
    assert_eq!(
        outcomes,
        [
            Outcome::Unresolved(Reason::InstanceTypeNotWritten),
            resolved("Settings", &[]),
            resolved("Arc<Pool>", &[]),
            Outcome::Unresolved(Reason::InstanceTypeNotWritten),
        ]
    );
}

#[test]
fn a_call_that_returns_a_factory_is_an_expression() {
    let outcomes = outcomes(r"registry! { provide(App, make_factory(config.clone())) }");
    assert_eq!(outcomes, [Outcome::Unresolved(Reason::FactoryIsAnExpression)]);
}

#[test]
fn a_block_that_captures_clones_is_read_through_to_its_closure() {
    let outcomes = outcomes(
        r"registry! {
            provide(App, {
                let counter = counter.clone();
                move |Inject(config): Inject<Config>| -> InstantiatorResult<Reset> { counter.tick(); Ok(Reset) }
            }),
            provide(App, {
                let counter = counter.clone();
                move || Ok::<_, InstantiateErrorKind>(Reset)
            }),
        }",
    );
    assert_eq!(
        outcomes,
        [
            resolved("Reset", &[("Config", Mode::Inject)]),
            Outcome::Unresolved(Reason::ClosureReturnTypeNotWritten),
        ]
    );
}
