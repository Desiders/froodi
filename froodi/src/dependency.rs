use alloc::collections::btree_set::BTreeSet;

use crate::any::TypeInfo;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Dependency {
    pub type_info: TypeInfo,
}

pub(crate) const EMPTY_DEPENDENCIES: BTreeSet<Dependency> = BTreeSet::new();
