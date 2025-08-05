use alloc::{collections::BTreeMap, sync::Arc};
use core::any::{Any, TypeId};

pub(crate) type Map = BTreeMap<TypeId, Arc<dyn Any + Send + Sync>>;
