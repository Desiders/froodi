use froodi::{utils::Merge as _, Registry};

use crate::entry_getters::__ENTRY_GETTERS;

pub trait AutoRegistries {
    #[must_use]
    fn provide_auto_registries(self) -> Self;
}

impl AutoRegistries for Registry {
    #[inline]
    fn provide_auto_registries(self) -> Self {
        __ENTRY_GETTERS.iter().fold(self, |registry, getter| registry.merge(getter()))
    }
}

#[cfg(feature = "async")]
pub(crate) mod async_impl {
    use super::AutoRegistries;
    use crate::entry_getters::__ASYNC_ENTRY_GETTERS;

    use froodi::{
        async_impl::{self, RegistryWithSync},
        utils::Merge as _,
    };

    pub trait AutoRegistriesWithSync {
        #[must_use]
        fn provide_auto_registries_with_sync(self) -> Self;
    }

    impl AutoRegistries for async_impl::Registry {
        #[inline]
        fn provide_auto_registries(self) -> Self {
            __ASYNC_ENTRY_GETTERS.iter().fold(self, |registry, getter| registry.merge(getter()))
        }
    }

    impl AutoRegistries for RegistryWithSync {
        #[inline]
        fn provide_auto_registries(self) -> Self {
            __ASYNC_ENTRY_GETTERS.iter().fold(self, |registry, getter| registry.merge(getter()))
        }
    }

    impl AutoRegistriesWithSync for RegistryWithSync {
        #[inline]
        fn provide_auto_registries_with_sync(self) -> Self {
            let registry = self.registry.provide_auto_registries();
            let sync = self.sync.provide_auto_registries();
            Self { registry, sync }
        }
    }
}

#[cfg(feature = "async")]
pub use async_impl::AutoRegistriesWithSync;
