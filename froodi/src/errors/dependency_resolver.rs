use alloc::boxed::Box;

use super::{instantiate::InstantiateErrorKind, instantiator::InstantiatorErrorKind};
use crate::{any::TypeInfo, scope::ScopeData};

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
    #[error(transparent)]
    Instantiator(InstantiatorErrorKind<Box<ResolveErrorKind>, InstantiateErrorKind>),
}
