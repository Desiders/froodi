#[derive(thiserror::Error, Debug)]
pub enum ScopeErrorKind {
    #[error("Child registries not found in container")]
    NoChildRegistries,
    #[error("Non-skipped registries not found in container. Registries with skipped scope aren't used by default.")]
    NoNonSkippedRegistries,
}

#[derive(thiserror::Error, Debug)]
pub enum ScopeWithErrorKind {
    #[error("Child registries not found in container")]
    NoChildRegistries,
    #[error("Registry with name {name} and priority {priority} not found in container")]
    NoChildRegistriesWithScope { name: &'static str, priority: u8 },
}
