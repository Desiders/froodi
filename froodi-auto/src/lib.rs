mod registry;

pub mod entry_getters;

pub use registry::AutoRegistries;
#[cfg(feature = "async")]
pub use registry::AutoRegistriesWithSync;

#[cfg(feature = "macros")]
pub use froodi_auto_macros::injectable;
