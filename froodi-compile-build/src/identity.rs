//! Deciding whether the written type spellings identify types.
//!
//! The analysis compares types by spelling. One spelling is taken to denote one type across all
//! analysed sources. A spelling that names a `type` alias of the analysed sources, or a path
//! whose final name is also reached by another path spelling (`Config`, `crate::Config`,
//! `settings::Config`), downgrades every registration that uses it to
//! [`Reason::TypeIsOnlyASpelling`].

use std::collections::{BTreeMap, BTreeSet};

use syn::{
    visit::{self, Visit},
    Type,
};

use crate::model::{Outcome, Reason, Registration, Registry, SpellingIssue};

/// A path as written without generic arguments, and its final name.
struct PathSpelling {
    full: String,
    name: String,
    segments: usize,
}

fn paths(ty: &str) -> Vec<PathSpelling> {
    struct Paths(Vec<PathSpelling>);

    impl Visit<'_> for Paths {
        fn visit_path(&mut self, path: &syn::Path) {
            let names: Vec<_> = path.segments.iter().map(|segment| segment.ident.to_string()).collect();
            if let Some(name) = names.last() {
                let prefix = if path.leading_colon.is_some() { "::" } else { "" };
                self.0.push(PathSpelling {
                    full: format!("{prefix}{}", names.join("::")),
                    name: name.clone(),
                    segments: names.len(),
                });
            }
            visit::visit_path(self, path);
        }
    }

    let mut found = Paths(Vec::new());
    if let Ok(ty) = syn::parse_str::<Type>(ty) {
        found.visit_type(&ty);
    }
    found.0
}

fn written_types(registration: &Registration) -> Vec<&str> {
    match &registration.outcome {
        Outcome::Resolved { provides, deps } => std::iter::once(provides.as_str())
            .chain(deps.iter().map(|dependency| dependency.ty.as_str()))
            .collect(),
        Outcome::Unresolved(_) => Vec::new(),
    }
}

/// Downgrades the registrations of `registries` whose types are only spellings. `aliases` maps
/// each `type` alias name of the analysed sources to its aliased type.
pub(crate) fn settle(registries: &mut [&mut Registry], aliases: &BTreeMap<String, String>) {
    let mut spellings: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for registry in registries.iter_mut() {
        registry.for_each_registration_mut(&mut |registration| {
            for ty in written_types(registration) {
                for path in paths(ty) {
                    spellings.entry(path.name).or_default().insert(path.full);
                }
            }
        });
    }

    for registry in registries.iter_mut() {
        registry.for_each_registration_mut(&mut |registration| {
            let issue = written_types(registration).into_iter().find_map(|ty| {
                paths(ty).into_iter().find_map(|path| {
                    let issue = match aliases.get(&path.name) {
                        Some(target) if path.segments == 1 => SpellingIssue::Alias {
                            alias: path.name,
                            target: target.clone(),
                        },
                        _ => {
                            let shared = &spellings[&path.name];
                            if shared.len() < 2 {
                                return None;
                            }
                            SpellingIssue::SharedName {
                                spellings: shared.iter().cloned().collect(),
                            }
                        }
                    };
                    Some((ty.to_owned(), issue))
                })
            });
            if let Some((ty, issue)) = issue {
                registration.outcome = Outcome::Unresolved(Reason::TypeIsOnlyASpelling { ty, issue });
            }
        });
    }
}
