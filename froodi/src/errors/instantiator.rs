use alloc::{boxed::Box, fmt};
use core::{
    any::TypeId,
    fmt::{Display, Formatter},
};

#[derive(thiserror::Error, Debug)]
pub enum InstantiatorErrorKind<DepsErr, FactoryErr> {
    #[error(transparent)]
    Deps(DepsErr),
    #[error(transparent)]
    Factory(FactoryErr),
}

#[derive(thiserror::Error, Debug)]
pub enum DFSErrorKind {
    CyclicDependency { graph: (TypeId, Box<[TypeId]>) },
}

impl Display for DFSErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            DFSErrorKind::CyclicDependency {
                graph: (type_id, type_ids),
            } => {
                write!(f, "Cyclic dependency detected: ")?;
                write!(f, "{type_id:?} ")?;
                for type_id in type_ids {
                    write!(f, "->")?;
                    write!(f, "{type_id:?} ")?;
                }
            }
        }
        Ok(())
    }
}
