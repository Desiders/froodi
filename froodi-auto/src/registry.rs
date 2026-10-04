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
pub trait AutoRegistriesWithSync {
    #[must_use]
    fn provide_auto_registries_with_sync(self) -> Self;
}

#[cfg(feature = "async")]
impl AutoRegistriesWithSync for Registry {
    #[inline]
    fn provide_auto_registries_with_sync(self) -> Self {
        use crate::entry_getters::__ASYNC_ENTRY_GETTERS;

        let registry = self.provide_auto_registries();
        __ASYNC_ENTRY_GETTERS.iter().fold(registry, |registry, getter| registry.merge(getter()))
    }
}
