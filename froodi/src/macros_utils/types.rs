use crate::{any::TypeInfo, registry::InstantiatorData, Registry};

pub enum RegistryOrEntry {
    Registry(Registry),
    Entry((TypeInfo, InstantiatorData)),
}
