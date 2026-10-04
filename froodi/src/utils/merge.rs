use crate::{any::TypeInfo, macros_utils::types::RegistryOrEntry, registry::InstantiatorData, Registry};

pub trait Merge<T> {
    type Output;

    #[must_use]
    fn merge(self, other: T) -> Self::Output;
}

impl Merge<Registry> for Registry {
    type Output = Registry;

    #[inline]
    fn merge(mut self, other: Registry) -> Self::Output {
        self.extend(other);
        self
    }
}

impl Merge<(TypeInfo, InstantiatorData)> for Registry {
    type Output = Registry;

    #[inline]
    fn merge(mut self, (key, value): (TypeInfo, InstantiatorData)) -> Self::Output {
        self.insert(key, crate::registry::Selected::Sync(value));
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

#[cfg(feature = "async")]
impl Merge<(TypeInfo, crate::registry::AsyncInstantiatorData)> for Registry {
    type Output = Self;

    fn merge(mut self, (key, value): (TypeInfo, crate::registry::AsyncInstantiatorData)) -> Self {
        self.insert(key, crate::registry::Selected::Async(value));
        self
    }
}
