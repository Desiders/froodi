use crate::any::TypeInfo;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Dependency {
    pub type_info: TypeInfo,
}
