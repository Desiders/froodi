#[derive(thiserror::Error, Debug)]
pub enum InstantiatorErrorKind<DepsErr, FactoryErr> {
    #[error(transparent)]
    Deps(DepsErr),
    #[error(transparent)]
    Factory(FactoryErr),
}
