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
            DFSErrorKind::CyclicDependency {
                graph: (type_info, type_infos),
            } => {
                write!(f, "Cyclic dependency detected: {}", type_info.short_name_without_path())?;
                for type_info in type_infos {
                    write!(f, " -> {} ({})", type_info.short_name_without_path(), type_info.short_name())?;
                }
                write!(f, " -> {} ({})", type_info.short_name_without_path(), type_info.name)
            }
        }
    }
}
