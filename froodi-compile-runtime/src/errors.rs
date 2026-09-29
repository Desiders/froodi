//! Error types with the same shape as Froodi's, so code matching on them ports unchanged.

use alloc::boxed::Box;
use core::any::TypeId;
use core::fmt::{self, Display, Formatter};

use crate::scope::ScopeData;

/// A type as it appears in errors: its id and its name.
#[derive(Debug, Clone, Copy)]
pub struct TypeInfo {
    pub name: &'static str,
    pub id: TypeId,
}

impl TypeInfo {
    #[inline]
    #[must_use]
    pub fn of<T: ?Sized + 'static>() -> Self {
        Self {
            name: core::any::type_name::<T>(),
            id: TypeId::of::<T>(),
        }
    }
}

impl PartialEq for TypeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for TypeInfo {}

impl Display for TypeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum InstantiateErrorKind {
    #[error(transparent)]
    Custom(#[from] anyhow::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum InstantiatorErrorKind<DepsErr, FactoryErr> {
    #[error(transparent)]
    Deps(DepsErr),
    #[error(transparent)]
    Factory(FactoryErr),
}

#[derive(thiserror::Error, Debug)]
pub enum ResolveErrorKind {
    #[error("Instantiator for {type_info:?} not found in registry")]
    NoInstantiator { type_info: TypeInfo },
    #[error(
        "\
        Instantiator no accessible. \
        You can't access the instantiator from child scope. \
        Actual scope: {} ({} priority), expected: {} ({} priority)\
        ",
        actual_scope_data.name, actual_scope_data.priority,
        expected_scope_data.name, expected_scope_data.priority,
    )]
    NoAccessible {
        expected_scope_data: ScopeData,
        actual_scope_data: ScopeData,
    },
    #[error("Incorrect instantiator provides type. Actual: {actual:?}, expected: {expected:?}")]
    IncorrectType { expected: TypeInfo, actual: TypeInfo },
    #[error("{type_info} is registered as a context value, but the context of scope {scope} does not contain it")]
    NoContextValue { type_info: TypeInfo, scope: &'static str },
    #[error("{type_info} has an async factory; resolve it through the async container")]
    AsyncOnly { type_info: TypeInfo },
    #[error(transparent)]
    Instantiator(InstantiatorErrorKind<Box<ResolveErrorKind>, InstantiateErrorKind>),
}

#[derive(thiserror::Error, Debug)]
pub enum ScopeErrorKind {
    #[error("Child registries not found in container")]
    NoChildRegistries,
    #[error("Non-skipped registries not found in container. Registries with skipped scope aren't used by default.")]
    NoNonSkippedRegistries,
}

#[derive(thiserror::Error, Debug)]
pub enum ScopeWithErrorKind {
    #[error("Child registries not found in container")]
    NoChildRegistries,
    #[error("Registry with name {name} and priority {priority} not found in container")]
    NoChildRegistriesWithScope { name: &'static str, priority: u8 },
}

/// Result alias used by factories, as in Froodi.
pub type InstantiatorResult<T, E = InstantiateErrorKind> = Result<T, E>;
