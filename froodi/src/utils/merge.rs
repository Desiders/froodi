use crate::{any::TypeInfo, macros_utils::types::RegistryOrEntry, registry::InstantiatorData, utils::hlist, Registry};

pub trait Merge<T> {
    type Output;

    #[must_use]
    fn merge(self, other: T) -> Self::Output;
}

impl Merge<Registry> for Registry {
    type Output = Registry;

    #[inline]
    fn merge(mut self, other: Registry) -> Self::Output {
        self.entries.extend(other.entries);
        self
    }
}

impl Merge<(TypeInfo, InstantiatorData)> for Registry {
    type Output = Registry;

    #[inline]
    fn merge(mut self, (key, value): (TypeInfo, InstantiatorData)) -> Self::Output {
        self.entries.insert(key, value);
        self
    }
}

impl Merge<RegistryOrEntry> for Registry {
    type Output = Self;

    #[inline]
    fn merge(self, registry_or_entry: RegistryOrEntry) -> Self::Output {
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
    type Output = Self;

    #[inline]
    fn merge(self, other: H) -> Self::Output {
        other.into_iter().fold(self, Merge::merge)
    }
}

#[cfg(feature = "async")]
mod async_impl {
    use super::{hlist, Merge, Registry, TypeInfo};
    use crate::{
        async_impl::{self, RegistryWithSync},
        macros_utils::types::{
            RegistryKind::{self, Async, AsyncWithSync, Sync},
            RegistryKindOrEntry::{self, Entry, Kind},
        },
    };

    impl Merge<async_impl::Registry> for async_impl::Registry {
        type Output = Self;

        #[inline]
        fn merge(mut self, registry: Self) -> Self::Output {
            self.entries.extend(registry.entries);
            self
        }
    }

    impl Merge<Registry> for async_impl::Registry {
        type Output = RegistryWithSync;

        #[inline]
        fn merge(self, sync: Registry) -> Self::Output {
            Self::Output { registry: self, sync }
        }
    }

    impl Merge<async_impl::Registry> for Registry {
        type Output = RegistryWithSync;

        #[inline]
        fn merge(self, registry: async_impl::Registry) -> Self::Output {
            Self::Output { registry, sync: self }
        }
    }

    impl Merge<RegistryWithSync> for RegistryWithSync {
        type Output = Self;

        #[inline]
        fn merge(mut self, registry: RegistryWithSync) -> Self::Output {
            self.sync.entries.extend(registry.sync.entries);
            self.registry.entries.extend(registry.registry.entries);
            self
        }
    }

    impl Merge<Registry> for RegistryWithSync {
        type Output = Self;

        #[inline]
        fn merge(mut self, registry: Registry) -> Self::Output {
            self.sync.entries.extend(registry.entries);
            self
        }
    }

    impl Merge<async_impl::Registry> for RegistryWithSync {
        type Output = Self;

        #[inline]
        fn merge(mut self, registry: async_impl::Registry) -> Self::Output {
            self.registry.entries.extend(registry.entries);
            self
        }
    }

    impl Merge<(TypeInfo, async_impl::InstantiatorData)> for Registry {
        type Output = RegistryWithSync;

        #[inline]
        fn merge(self, (key, value): (TypeInfo, async_impl::InstantiatorData)) -> Self::Output {
            let mut registry = async_impl::Registry::default();
            registry.entries.insert(key, value);
            Self::Output { registry, sync: self }
        }
    }

    impl Merge<(TypeInfo, async_impl::InstantiatorData)> for async_impl::Registry {
        type Output = Self;

        #[inline]
        fn merge(mut self, (key, value): (TypeInfo, async_impl::InstantiatorData)) -> Self::Output {
            self.entries.insert(key, value);
            self
        }
    }

    impl Merge<(TypeInfo, async_impl::InstantiatorData)> for RegistryWithSync {
        type Output = Self;

        #[inline]
        fn merge(mut self, (key, value): (TypeInfo, async_impl::InstantiatorData)) -> Self::Output {
            self.registry.entries.insert(key, value);
            self
        }
    }

    impl Merge<Registry> for RegistryKind {
        type Output = Self;

        #[inline]
        fn merge(self, registry: Registry) -> Self::Output {
            match self {
                Sync(other) => Sync(registry.merge(other)),
                Async(other) => AsyncWithSync(registry.merge(other)),
                AsyncWithSync(other) => AsyncWithSync(other.merge(registry)),
            }
        }
    }

    impl Merge<async_impl::Registry> for RegistryKind {
        type Output = Self;

        #[inline]
        fn merge(self, registry: async_impl::Registry) -> Self::Output {
            match self {
                Sync(other) => AsyncWithSync(other.merge(registry)),
                Async(other) => Async(registry.merge(other)),
                AsyncWithSync(other) => AsyncWithSync(other.merge(registry)),
            }
        }
    }

    impl Merge<RegistryWithSync> for RegistryKind {
        type Output = Self;

        #[inline]
        fn merge(self, registry: RegistryWithSync) -> Self::Output {
            match self {
                Sync(other) => AsyncWithSync(registry.merge(other)),
                Async(other) => AsyncWithSync(registry.merge(other)),
                AsyncWithSync(other) => AsyncWithSync(registry.merge(other)),
            }
        }
    }

    impl Merge<RegistryKindOrEntry> for RegistryWithSync {
        type Output = Self;

        #[inline]
        fn merge(self, registry_kind_or_entry: RegistryKindOrEntry) -> Self::Output {
            match registry_kind_or_entry {
                Kind(Sync(registry)) => self.merge(registry),
                Kind(Async(registry)) => self.merge(registry),
                Kind(AsyncWithSync(registry)) => self.merge(registry),
                Entry(entry) => self.merge(entry),
            }
        }
    }

    impl Merge<RegistryKindOrEntry> for async_impl::Registry {
        type Output = RegistryWithSync;

        #[inline]
        fn merge(self, registry_kind_or_entry: RegistryKindOrEntry) -> Self::Output {
            match registry_kind_or_entry {
                Kind(Sync(registry)) => self.merge(registry),
                Kind(Async(registry)) => self.merge(registry).into(),
                Kind(AsyncWithSync(registry)) => registry.merge(self),
                Entry(entry) => self.merge(entry).into(),
            }
        }
    }

    impl Merge<RegistryKindOrEntry> for RegistryKind {
        type Output = Self;

        #[inline]
        fn merge(self, registry_kind_or_entry: RegistryKindOrEntry) -> Self::Output {
            match (self, registry_kind_or_entry) {
                (Sync(registry), Kind(Sync(other))) => Sync(registry.merge(other)),
                (Async(registry), Kind(Async(other))) => Async(registry.merge(other)),
                (AsyncWithSync(registry), Kind(AsyncWithSync(other))) => AsyncWithSync(registry.merge(other)),
                (Sync(registry), Entry(entry)) => AsyncWithSync(registry.merge(entry)),
                (Async(registry), Entry(entry)) => Async(registry.merge(entry)),
                (AsyncWithSync(registry), Entry(entry)) => AsyncWithSync(registry.merge(entry)),
                (Sync(registry), Kind(AsyncWithSync(other))) => AsyncWithSync(other.merge(registry)),
                (Async(registry), Kind(Sync(other))) => AsyncWithSync(other.merge(registry)),
                (AsyncWithSync(registry), Kind(Sync(other))) => AsyncWithSync(registry.merge(other)),
                (Sync(registry), Kind(Async(other))) => AsyncWithSync(registry.merge(other)),
                (Async(registry), Kind(AsyncWithSync(other))) => AsyncWithSync(other.merge(registry)),
                (AsyncWithSync(registry), Kind(Async(other))) => AsyncWithSync(registry.merge(other)),
            }
        }
    }

    impl<H> Merge<H> for RegistryWithSync
    where
        H: hlist::IntoIterator<RegistryKindOrEntry>,
    {
        type Output = Self;

        fn merge(self, other: H) -> Self::Output {
            other.into_iter().fold(self, Merge::merge)
        }
    }
}
