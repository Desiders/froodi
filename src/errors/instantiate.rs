#[derive(thiserror::Error, Debug)]
pub(crate) enum InstantiateErrorKind {
    #[error(transparent)]
    Custom(#[from] anyhow::Error),
}
