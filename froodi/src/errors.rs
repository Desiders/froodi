mod container;
mod dependency_resolver;
mod instantiate;
mod instantiator;

pub use container::{ScopeErrorKind, ScopeWithErrorKind};
pub use dependency_resolver::ResolveErrorKind;
pub use instantiate::InstantiateErrorKind;
pub use instantiator::InstantiatorErrorKind;

use crate::dependency_resolver::DependencyResolver;

#[allow(type_alias_bounds)]
pub type InstantiatorResult<Dep: DependencyResolver<Error = Err>, Err: Into<ResolveErrorKind> = InstantiateErrorKind> = Result<Dep, Err>;
