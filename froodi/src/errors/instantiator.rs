use alloc::{boxed::Box, fmt};
use core::fmt::{Debug, Display, Formatter};

use crate::{any::TypeInfo, scope::ScopeData};

#[derive(thiserror::Error, Debug)]
pub enum InstantiatorErrorKind<DepsErr, FactoryErr> {
    #[error(transparent)]
    Deps(DepsErr),
    #[error(transparent)]
    Factory(FactoryErr),
}

#[derive(thiserror::Error)]
pub enum ValidationErrorKind {
    CyclicDependency {
        graph: (TypeInfo, Box<[TypeInfo]>),
    },
    UnreachableDependency {
        dependent: TypeInfo,
        dependent_scope: ScopeData,
        dependency: TypeInfo,
        dependency_scope: ScopeData,
    },
}

impl Debug for ValidationErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for ValidationErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ValidationErrorKind::CyclicDependency { graph } => {
                let (type_info, rest) = graph;
                let short_name = type_info.short_name();
                write!(f, "Cyclic dependency detected:\n{short_name} ")?;
                for type_info in rest.iter() {
                    write!(f, "\n↳ depends on {} ({})", type_info.short_name(), type_info.name)?;
                }
                writeln!(f, "\n ↳ depends on {} ({})", short_name, type_info.name)
            }
            ValidationErrorKind::UnreachableDependency {
                dependent,
                dependent_scope,
                dependency,
                dependency_scope,
            } => write!(
                f,
                "Unreachable dependency: `{}` (scope `{}`, priority {}) depends on `{}` (scope `{}`, priority {}), \
                 which is a narrower scope and can never be resolved from it. A dependency must live in an equal or wider scope.",
                dependent.short_name(),
                dependent_scope.name,
                dependent_scope.priority,
                dependency.short_name(),
                dependency_scope.name,
                dependency_scope.priority,
            ),
        }
    }
}
