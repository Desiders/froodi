use alloc::boxed::Box;

use super::{instantiate::InstantiateErrorKind, instantiator::InstantiatorErrorKind};

#[derive(thiserror::Error, Debug)]
pub(crate) enum ResolveErrorKind {
    #[error("Factory not found")]
    NoFactory,
    #[error(transparent)]
    Instantiator(InstantiatorErrorKind<Box<ResolveErrorKind>, InstantiateErrorKind>),
}
