use core::any::TypeId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Dependency {
    pub type_id: TypeId,
}
