//! A source-text analysis of `registry!` invocations, the part of a registry graph a `build.rs`
//! can compute.
//!
//! # Cargo staging
//!
//! Cargo compiles and runs a package's build script before it compiles any other target of that
//! package. The build script is an ordinary host binary: its inputs are its build-dependencies
//! and the files on disk. Macro expansion and type checking of the package happen later, inside
//! the rustc invocation that compiles the library or binary. So when a `build.rs` runs:
//!
//! - the package's source exists only as text, and [`analyze`] takes text;
//! - output of proc macros applied to the package's own source does not exist yet. An attribute
//!   such as `#[injectable]` is visible as written, and the items and registrations it expands
//!   to are not, whether the proc macro lives in another package or is a build-dependency;
//! - output of `macro_rules!` does not exist either, so a `registry!` written inside a macro
//!   body is a token tree, not an invocation;
//! - no name resolution or type inference has run, so a type is known by its spelling only.
//!
//! # What the analysis reads
//!
//! For every `provide(...)` it recovers the provided type and the dependency types with their
//! mode when the source spells them:
//!
//! - a free function declared in the enclosing module or block, found by a bare name or
//!   `self::name`: parameters `Inject<T>` / `InjectTransient<T>` and a return type
//!   `Result<T, _>` / `InstantiatorResult<T>`;
//! - a closure whose parameters are annotated and whose return type is annotated or spelled by
//!   an `Ok::<T, _>(...)` tail, directly or in a returned `async` block;
//! - `instance::<T>(...)` and `instance(Name { ... })`.
//!
//! Everything else is [`Outcome::Unresolved`] with a [`Reason`]. Types are compared by spelling;
//! the rules are in [`Reason::TypeIsOnlyASpelling`] and [`SpellingIssue`].

mod collect;
mod identity;
mod model;
mod render;
mod resolve;
mod syntax;

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

pub use model::{
    Analysis, Dependency, DirAnalysis, Extension, ExtensionOutcome, Mode, Outcome, Reason, Registration, Registry, RegistryKind,
    SpellingIssue, Summary,
};

/// Analyses every `registry!` and `async_registry!` invocation of one Rust source file.
///
/// Type identity is settled within this source: see [`SpellingIssue`].
///
/// # Errors
///
/// When `source` is not a Rust file, or a `registry!` body is not registry syntax.
pub fn analyze(source: &str) -> syn::Result<Analysis> {
    let mut collected = collect::collect(&syn::parse_file(source)?)?;
    let mut registries: Vec<_> = collected.registries.iter_mut().collect();
    identity::settle(&mut registries, &collected.aliases);
    Ok(Analysis {
        registries: collected.registries,
    })
}

/// Analyses every `.rs` file under `dir`, recursively, in path order. Type identity is settled
/// across all parsed files together, with the `type` aliases of all of them.
///
/// # Errors
///
/// When a directory or file cannot be read. A file that does not parse is listed in
/// [`DirAnalysis::unparsed`].
pub fn analyze_dir(dir: &Path) -> io::Result<DirAnalysis> {
    let mut paths = Vec::new();
    rust_files(dir, &mut paths)?;
    paths.sort();

    let mut parsed = Vec::new();
    let mut unparsed = Vec::new();
    let mut aliases = BTreeMap::new();
    for path in paths {
        let source = fs::read_to_string(&path)?;
        match syn::parse_file(&source).and_then(|file| collect::collect(&file)) {
            Ok(collected) => {
                aliases.extend(collected.aliases);
                parsed.push((path, collected.registries));
            }
            Err(error) => unparsed.push((path, error)),
        }
    }

    let mut registries: Vec<_> = parsed.iter_mut().flat_map(|(_, registries)| registries.iter_mut()).collect();
    identity::settle(&mut registries, &aliases);
    Ok(DirAnalysis {
        files: parsed
            .into_iter()
            .map(|(path, registries)| (path, Analysis { registries }))
            .collect(),
        unparsed,
    })
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            rust_files(&path, out)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
    Ok(())
}
