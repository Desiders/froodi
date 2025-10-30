use alloc::{boxed::Box, fmt};
use core::fmt::{Debug, Display, Formatter};

use crate::any::TypeInfo;

#[derive(thiserror::Error, Debug)]
pub enum InstantiatorErrorKind<DepsErr, FactoryErr> {
    #[error(transparent)]
    Deps(DepsErr),
    #[error(transparent)]
    Factory(FactoryErr),
}

#[derive(thiserror::Error)]
pub enum DFSErrorKind {
    CyclicDependency { graph: (TypeInfo, Box<[TypeInfo]>) },
}

impl Debug for DFSErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for DFSErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            DFSErrorKind::CyclicDependency { graph } => {
                let (type_info, rest) = graph;
                let short_name = type_info.short_name_without_path();
                write!(f, "Cyclic dependency detected:\n{} ", short_name)?;
                for type_info in rest.iter() {
                    write!(f, "\n↳ depends on {} ({})", type_info.short_name_without_path(), type_info.name)?;
                }
                writeln!(f, "\n ↳ depends on {} ({})", short_name, type_info.name)
            }
        }
    }
}
