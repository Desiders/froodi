//! Type identity: a build step sees spellings, and several spellings may denote one type.

use froodi_compile_build::{analyze, Outcome, Reason, SpellingIssue};

fn outcomes(source: &str) -> Vec<Outcome> {
    analyze(source)
        .unwrap()
        .registrations()
        .into_iter()
        .map(|registration| registration.outcome.clone())
        .collect()
}

fn spelling_issue(outcome: &Outcome) -> (&str, &SpellingIssue) {
    match outcome {
        Outcome::Unresolved(Reason::TypeIsOnlyASpelling { ty, issue }) => (ty, issue),
        other => panic!("expected TypeIsOnlyASpelling, got {other:?}"),
    }
}

#[test]
fn a_type_alias_declared_in_the_file_is_reported_with_its_target() {
    let source = r"
        type Db = Arc<Database>;

        fn make_database() -> InstantiatorResult<Db> { todo!() }
        fn make_repo(Inject(db): Inject<Db>) -> InstantiatorResult<Repo> { todo!() }
        fn make_cache() -> InstantiatorResult<Cache> { todo!() }

        fn build() -> Registry {
            registry! {
                provide(App, make_database),
                provide(App, make_repo),
                provide(App, make_cache),
            }
        }
    ";
    let outcomes = outcomes(source);
    let alias = SpellingIssue::Alias {
        alias: "Db".to_owned(),
        target: "Arc<Database>".to_owned(),
    };
    assert_eq!(spelling_issue(&outcomes[0]), ("Db", &alias));
    assert_eq!(spelling_issue(&outcomes[1]), ("Db", &alias));
    assert!(matches!(outcomes[2], Outcome::Resolved { .. }));
}

#[test]
fn paths_that_end_in_the_same_name_are_not_identified() {
    let source = r"
        fn make_config() -> InstantiatorResult<crate::Config> { todo!() }
        fn make_server(Inject(config): Inject<Config>) -> InstantiatorResult<Server> { todo!() }
        fn make_worker(Inject(config): Inject<Arc<settings::Config>>) -> InstantiatorResult<Worker> { todo!() }
        fn make_clock() -> InstantiatorResult<Clock> { todo!() }

        fn build() -> Registry {
            registry! {
                provide(App, make_config),
                provide(App, make_server),
                provide(App, make_worker),
                provide(App, make_clock),
            }
        }
    ";
    let outcomes = outcomes(source);
    let shared = SpellingIssue::SharedName {
        spellings: vec!["Config".to_owned(), "crate::Config".to_owned(), "settings::Config".to_owned()],
    };
    assert_eq!(spelling_issue(&outcomes[0]), ("crate::Config", &shared));
    assert_eq!(spelling_issue(&outcomes[1]), ("Config", &shared));
    assert_eq!(spelling_issue(&outcomes[2]), ("Arc<settings::Config>", &shared));
    assert!(matches!(outcomes[3], Outcome::Resolved { .. }));
}

#[test]
fn one_spelling_used_everywhere_is_identified_and_whitespace_does_not_matter() {
    let source = r"
        fn make_pool() -> InstantiatorResult<Arc<Pool>> { todo!() }
        fn make_repo(Inject(pool): Inject< Arc< Pool > >) -> InstantiatorResult<Repo> { todo!() }

        fn build() -> Registry {
            registry! {
                provide(App, make_pool),
                provide(App, make_repo),
            }
        }
    ";
    let outcomes = outcomes(source);
    assert!(matches!(&outcomes[0], Outcome::Resolved { provides, .. } if provides == "Arc<Pool>"));
    assert!(matches!(&outcomes[1], Outcome::Resolved { deps, .. } if deps[0].ty == "Arc<Pool>"));
}

#[test]
fn a_std_path_and_its_imported_name_are_two_spellings_too() {
    let source = r"
        use std::sync::Arc;

        fn make_pool() -> InstantiatorResult<std::sync::Arc<Pool>> { todo!() }
        fn make_repo(Inject(pool): Inject<Arc<Pool>>) -> InstantiatorResult<Repo> { todo!() }

        fn build() -> Registry {
            registry! {
                provide(App, make_pool),
                provide(App, make_repo),
            }
        }
    ";
    let outcomes = outcomes(source);
    let shared = SpellingIssue::SharedName {
        spellings: vec!["Arc".to_owned(), "std::sync::Arc".to_owned()],
    };
    assert_eq!(spelling_issue(&outcomes[0]), ("std::sync::Arc<Pool>", &shared));
    assert_eq!(spelling_issue(&outcomes[1]), ("Arc<Pool>", &shared));
}
