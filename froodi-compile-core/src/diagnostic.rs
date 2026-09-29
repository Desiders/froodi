//! Graph diagnostics. Each one names the registrations involved and, where it helps, the
//! dependency path that leads to the problem.

use alloc::{string::String, vec::Vec};
use core::fmt::{self, Display, Formatter};

use crate::ir::{Origin, ScopeKey};

/// One step of a dependency path: the type and where its registration was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathStep {
    pub type_name: &'static str,
    pub origin: Option<Origin>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Diagnostic {
    /// A request targets a type nothing provides. `path` runs from a root registration down to
    /// the registration whose request is unresolved.
    MissingBinding { missing: &'static str, path: Vec<PathStep> },
    /// Two or more registrations provide the same binding.
    Duplicate {
        type_name: &'static str,
        origins: Vec<Option<Origin>>,
    },
    /// A dependency cycle. The first step is repeated at the end.
    Cycle { path: Vec<PathStep> },
    /// A registration depends on a registration in a narrower scope, which it can never reach.
    ScopeViolation {
        dependent: PathStep,
        dependent_scope: ScopeKey,
        dependency: PathStep,
        dependency_scope: ScopeKey,
    },
    /// A registration uses a scope that is not part of the registry's scope hierarchy.
    UnknownScope { registration: PathStep, scope: ScopeKey },
}

/// All diagnostics of one compilation, in a deterministic order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostics(pub Vec<Diagnostic>);

fn short(name: &str) -> &str {
    // `a::b::C<d::E>` -> `C<d::E>`: cut the path before the first generic argument.
    let head = name.split('<').next().unwrap_or(name);
    match head.rfind("::") {
        Some(pos) => &name[pos + 2..],
        None => name,
    }
}

fn write_tree(f: &mut Formatter<'_>, path: &[PathStep], tail: Option<&str>) -> fmt::Result {
    let mut indent = String::new();
    for (depth, step) in path.iter().enumerate() {
        if depth == 0 {
            write!(f, "\n{}", short(step.type_name))?;
        } else {
            write!(f, "\n{indent}└── {}", short(step.type_name))?;
            indent.push_str("    ");
        }
        if let Some(origin) = step.origin {
            write!(f, "  [{origin}]")?;
        }
    }
    if let Some(tail) = tail {
        write!(f, "\n{indent}{tail}")?;
    }
    Ok(())
}

impl Display for Diagnostic {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Diagnostic::MissingBinding { missing, path } => {
                let root = path.first().map_or(*missing, |step| step.type_name);
                writeln!(f, "cannot construct {}", short(root))?;
                let mut full = path.clone();
                full.push(PathStep {
                    type_name: missing,
                    origin: None,
                });
                write_tree(f, &full, Some("no registration found"))
            }
            Diagnostic::Duplicate { type_name, origins } => {
                write!(f, "{} is provided by {} registrations", short(type_name), origins.len())?;
                for origin in origins {
                    match origin {
                        Some(origin) => write!(f, "\n  - {origin}")?,
                        None => f.write_str("\n  - <unknown origin>")?,
                    }
                }
                Ok(())
            }
            Diagnostic::Cycle { path } => {
                f.write_str("dependency cycle\n")?;
                write_tree(f, path, None)
            }
            Diagnostic::ScopeViolation {
                dependent,
                dependent_scope,
                dependency,
                dependency_scope,
            } => write!(
                f,
                "{} (scope {dependent_scope}) depends on {} (scope {dependency_scope}), which is a narrower scope \
                 and can never be resolved from it; a dependency must live in an equal or wider scope",
                short(dependent.type_name),
                short(dependency.type_name),
            ),
            Diagnostic::UnknownScope { registration, scope } => write!(
                f,
                "{} is registered in scope {scope}, which is not part of the registry's scope hierarchy",
                short(registration.type_name),
            ),
        }
    }
}

impl Display for Diagnostics {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for (i, diagnostic) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("\n\n")?;
            }
            write!(f, "error: {diagnostic}")?;
        }
        Ok(())
    }
}
