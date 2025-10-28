use crate::{any::TypeInfo, async_impl::RegistryWithSync, registry::InstantiatorData, Registry};

pub enum RegistryOrEntry {
    Registry(Registry),
    Entry((TypeInfo, InstantiatorData)),
}

#[cfg(feature = "async")]
mod async_impl {
    use super::{Registry, RegistryWithSync};
    use crate::{
        any::TypeInfo,
        async_impl::{self, registry::InstantiatorData},
    };

    pub enum RegistryKind {
        Sync(Registry),
        Async(async_impl::Registry),
        AsyncWithSync(RegistryWithSync),
    }

    pub enum RegistryKindOrEntry {
        Kind(RegistryKind),
        Entry((TypeInfo, InstantiatorData)),
    }
}

#[cfg(feature = "async")]
pub use async_impl::{RegistryKind, RegistryKindOrEntry};
