#[derive(thiserror::Error, Debug)]
pub enum InstantiateErrorKind {
    #[error(transparent)]
    Custom(#[from] anyhow::Error),
}
