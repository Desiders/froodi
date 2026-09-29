//! The Cargo staging constraint, stated over analysis results.
//!
//! Cargo runs a package's `build.rs` before rustc compiles that package's library or binaries.
//! Macro expansion (proc macros and `macro_rules!`), name resolution and type inference of the
//! package all happen inside that later rustc invocation. A build script therefore reads the
//! package's source as text, which is exactly the input of `analyze`. Each test below shows one
//! consequence on source that compiles and works with `froodi`.

use froodi_compile_build::{analyze, Outcome, Reason};

/// `examples/sync_auto_provide` of this workspace: two registrations come from
/// `#[injectable]`, a proc macro, and reach the container through `provide_auto_registries()`.
const AUTO_PROVIDE: &str = include_str!("../../examples/sync_auto_provide/src/main.rs");

#[test]
fn registrations_generated_by_a_proc_macro_are_not_in_the_text() {
    let analysis = analyze(AUTO_PROVIDE).unwrap();
    let factories: Vec<_> = analysis
        .registrations()
        .iter()
        .map(|registration| registration.factory.as_str())
        .collect();
    // The container this example builds resolves `UserRepository` and `StartHandler`; the
    // text holds only the attribute that will generate their registrations inside rustc.
    assert_eq!(factories, ["instance(db)"]);
    assert!(AUTO_PROVIDE.contains("#[injectable]"));
}

#[test]
fn a_factory_generated_by_a_proc_macro_is_not_in_the_file() {
    let source = r"
        #[derive(Factory)] // expands to `fn make_repository(...) -> InstantiatorResult<Repository>`
        struct Repository { db: Arc<Database> }

        fn build() -> Registry {
            registry! { provide(Request, make_repository) }
        }
    ";
    let analysis = analyze(source).unwrap();
    assert_eq!(
        analysis.registrations()[0].outcome,
        Outcome::Unresolved(Reason::FactoryNotInFile {
            path: "make_repository".to_owned()
        })
    );
}

#[test]
fn a_registry_written_inside_a_macro_rules_body_is_a_token_tree() {
    let source = r"
        macro_rules! wiring {
            ($scope:expr) => { registry! { provide($scope, make_cache) } };
        }

        fn build() -> Registry {
            wiring!(App)
        }
    ";
    assert!(analyze(source).unwrap().registries.is_empty());
}

#[test]
fn only_spellings_are_available_because_types_are_inferred_later() {
    // rustc resolves `db` to `Arc<Database>`, `settings` to `Settings` and the closure to
    // `Repo`; the text writes none of these types.
    let source = r"
        fn build(db: Arc<Database>, settings: Settings) -> Registry {
            registry! {
                provide(App, instance(db)),
                provide(App, instance(settings)),
                provide(Request, |Inject(db): Inject<Database>| Ok(Repo(db))),
            }
        }
    ";
    let reasons: Vec<_> = analyze(source)
        .unwrap()
        .registrations()
        .iter()
        .map(|registration| registration.outcome.clone())
        .collect();
    assert_eq!(
        reasons,
        [
            Outcome::Unresolved(Reason::InstanceTypeNotWritten),
            Outcome::Unresolved(Reason::InstanceTypeNotWritten),
            Outcome::Unresolved(Reason::ClosureReturnTypeNotWritten),
        ]
    );
}
