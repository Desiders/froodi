//! `analyze_dir`: every `.rs` file under a directory, with type identity settled across files.

use std::{fs, path::PathBuf};

use froodi_compile_build::{analyze_dir, Outcome, Reason, SpellingIssue};

fn fixture(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&root);
    for (path, text) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    root
}

#[test]
fn walks_rust_files_recursively_in_path_order_and_keeps_unparsed_files_apart() {
    let root = fixture(
        "walk",
        &[
            ("b.rs", "fn b() -> Registry { registry! { provide(App, instance(x)) } }"),
            ("nested/a.rs", "fn a() -> Registry { registry! { provide(App, make) } }"),
            ("broken.rs", "fn broken( {"),
            ("notes.txt", "registry! { provide(App, make) }"),
        ],
    );
    let analysis = analyze_dir(&root).unwrap();
    let files: Vec<_> = analysis.files.iter().map(|(path, _)| path.strip_prefix(&root).unwrap()).collect();
    assert_eq!(files, [PathBuf::from("b.rs"), PathBuf::from("nested/a.rs")]);
    let unparsed: Vec<_> = analysis
        .unparsed
        .iter()
        .map(|(path, _)| path.strip_prefix(&root).unwrap())
        .collect();
    assert_eq!(unparsed, [PathBuf::from("broken.rs")]);
    assert_eq!(analysis.summary().total(), 2);
}

#[test]
fn spellings_are_compared_across_files() {
    let root = fixture(
        "identity",
        &[
            (
                "config.rs",
                "fn make_config() -> InstantiatorResult<crate::Config> { todo!() }
                 fn a() -> Registry { registry! { provide(App, make_config) } }",
            ),
            (
                "server.rs",
                "fn make_server(Inject(config): Inject<Config>) -> InstantiatorResult<Server> { todo!() }
                 fn b() -> Registry { registry! { provide(App, make_server) } }",
            ),
        ],
    );
    let analysis = analyze_dir(&root).unwrap();
    let issue = SpellingIssue::SharedName {
        spellings: vec!["Config".to_owned(), "crate::Config".to_owned()],
    };
    assert_eq!(analysis.files.len(), 2);
    for (_, file) in &analysis.files {
        match &file.registrations()[0].outcome {
            Outcome::Unresolved(Reason::TypeIsOnlyASpelling { issue: found, .. }) => assert_eq!(found, &issue),
            other => panic!("expected TypeIsOnlyASpelling, got {other:?}"),
        }
    }
}
