use crate::{any::TypeInfo, macros_utils::types::RegistryOrEntry, registry::InstantiatorData, utils::hlist, Registry};

pub trait Merge<T> {
    #[must_use]
    fn merge(self, other: T) -> Self;
}

impl Merge<Registry> for Registry {
    #[inline]
    fn merge(mut self, other: Self) -> Self {
        self.entries.extend(other.entries);
        self
    }
}

impl Merge<(TypeInfo, InstantiatorData)> for Registry {
    #[inline]
    fn merge(mut self, (key, value): (TypeInfo, InstantiatorData)) -> Self {
        self.entries.insert(key, value);
        self
    }
}

impl Merge<RegistryOrEntry> for Registry {
    #[inline]
    fn merge(self, registry_or_entry: RegistryOrEntry) -> Self {
        match registry_or_entry {
            RegistryOrEntry::Registry(registry) => self.merge(registry),
            RegistryOrEntry::Entry(entry) => self.merge(entry),
        }
    }
}

impl<H> Merge<H> for Registry
where
    H: hlist::IntoIterator<RegistryOrEntry>,
{
    #[inline]
    fn merge(mut self, hlist: H) -> Self {
        for registry_or_entry in hlist.into_iter() {
            self = self.merge(registry_or_entry);
        }
        self
    }
}

#[cfg(feature = "async")]
mod async_impl {
    use super::{hlist, Merge, Registry, TypeInfo};
    use crate::{
        async_impl::{self, registry::InstantiatorData, RegistryWithSync},
        macros_utils::types::{RegistryKind, RegistryKindOrEntry},
    };

    impl Merge<RegistryWithSync> for RegistryWithSync {
        #[inline]
        fn merge(mut self, other: Self) -> Self {
            self.registry.entries.extend(other.registry.entries);
            self.sync = self.sync.merge(other.sync);
            self
        }
    }

    impl Merge<async_impl::Registry> for RegistryWithSync {
        #[inline]
        fn merge(mut self, other: async_impl::Registry) -> Self {
            self.registry.entries.extend(other.entries);
            self
        }
    }

    impl Merge<Registry> for RegistryWithSync {
        #[inline]
        fn merge(mut self, other: Registry) -> Self {
            self.sync = self.sync.merge(other);
            self
        }
    }

    impl Merge<(TypeInfo, InstantiatorData)> for RegistryWithSync {
        #[inline]
        fn merge(mut self, (key, value): (TypeInfo, InstantiatorData)) -> Self {
            self.registry.entries.insert(key, value);
            self
        }
    }

    impl Merge<RegistryKindOrEntry> for RegistryWithSync {
        #[inline]
        fn merge(self, registry_kind_or_entry: RegistryKindOrEntry) -> Self {
            match registry_kind_or_entry {
                RegistryKindOrEntry::Kind(RegistryKind::Sync(registry)) => self.merge(registry),
                RegistryKindOrEntry::Kind(RegistryKind::Async(registry)) => self.merge(registry),
                RegistryKindOrEntry::Kind(RegistryKind::AsyncWithSync(registry)) => self.merge(registry),
                RegistryKindOrEntry::Entry(entry) => self.merge(entry),
            }
        }
    }

    impl<H> Merge<H> for RegistryWithSync
    where
        H: hlist::IntoIterator<RegistryKindOrEntry>,
    {
        #[inline]
        fn merge(self, other: H) -> Self {
            other.into_iter().fold(self, RegistryWithSync::merge)
        }
    }
}
