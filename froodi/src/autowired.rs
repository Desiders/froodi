use crate::{any::TypeInfo, InstantiatorData};

pub use linkme::{self, distributed_slice};

#[distributed_slice]
pub static __GLOBAL_ENTRY_GETTERS: [fn() -> (TypeInfo, InstantiatorData)];
