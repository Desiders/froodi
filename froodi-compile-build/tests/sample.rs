//! The resolved/unresolved split on realistic registries.
//!
//! Run with `--nocapture` to print the split of the workspace's own examples, tests and benches.

use std::{collections::BTreeMap, path::Path};

use froodi_compile_build::{analyze, analyze_dir, Summary};

#[test]
fn the_service_fixture_splits_as_measured() {
    let summary = analyze(include_str!("fixtures/service.rs")).unwrap().summary();
    let expected = Summary {
        resolved: 7,
        unresolved: BTreeMap::from([
            ("ClosureReturnTypeNotWritten", 2),
            ("ExtendIsARuntimeExpression", 2),
            ("FactoryNotInFile", 2),
            ("GenericFactory", 1),
            ("InstanceTypeNotWritten", 2),
            ("TypeIsOnlyASpelling", 1),
        ]),
    };
    assert_eq!(summary, expected);
}

#[test]
fn the_workspace_registries_are_measured() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for dir in ["examples", "froodi/tests", "froodi/benches", "froodi-compile/tests"] {
        let analysis = analyze_dir(&workspace.join(dir)).unwrap();
        let summary = analysis.summary();
        eprintln!("{dir}: {summary}");
        for (path, error) in &analysis.unparsed {
            eprintln!("  unparsed {}: {error}", path.display());
            // trybuild fixtures whose registries are malformed on purpose.
            assert!(path.parent().is_some_and(|parent| parent.ends_with("ui")), "{}", path.display());
        }
        assert!(summary.total() > 0, "{dir} holds registries");
    }
}
