use alloc::boxed::Box;
use core::any::TypeId;

use super::{instantiate::InstantiateErrorKind, instantiator::InstantiatorErrorKind};

#[derive(thiserror::Error, Debug)]
pub enum ResolveErrorKind {
    #[error("Instantiator not found in registry")]
    NoInstantiator,
    #[error("Incorrect instantiator provides type. Actual: {actual:?}, expected: {expected:?}")]
    IncorrectType { expected: TypeId, actual: TypeId },
    #[error(transparent)]
    Instantiator(InstantiatorErrorKind<Box<ResolveErrorKind>, InstantiateErrorKind>),
}
