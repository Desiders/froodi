#[cfg(feature = "async")]
pub use crate::async_impl::typed_registration::async_reg;
pub use crate::registry::frontend::{IntoFragment, Registry};
pub use crate::registry::linking::{Empty, Node};
pub use crate::registry::registration::{reg, LocatedRegistration, NoFinalizer, RegistrationSource};
pub use crate::registry::static_scope::{ClassifyScope, ScopeType};
#[cfg(feature = "async")]
pub use froodi_macros::async_registry;
pub use froodi_macros::registry;
