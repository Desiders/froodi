use super::InstantiatorData;
use crate::any::TypeInfo;

pub use linkme::{self, distributed_slice};

#[distributed_slice]
pub static __GLOBAL_ASYNC_ENTRY_GETTERS: [fn() -> (TypeInfo, InstantiatorData)];
