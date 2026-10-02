#[cfg(feature = "async")]
pub use crate::compiled::async_impl::async_reg;
pub use crate::compiled::linking::{Empty, Node};
pub use crate::compiled::{reg, ClassifyScope, IntoFragment, LocatedRegistration, NoFinalizer, RegistrationSource, Registry, ScopeType};
#[cfg(feature = "async")]
pub use froodi_macros::compiled_async_registry as async_registry;
pub use froodi_macros::compiled_registry as registry;
