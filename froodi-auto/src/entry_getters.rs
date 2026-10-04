#[cfg(feature = "async")]
use froodi::AsyncInstantiatorData;
use froodi::{InstantiatorData, TypeInfo};

pub use linkme::{self, distributed_slice};

#[distributed_slice]
pub static __ENTRY_GETTERS: [fn() -> (TypeInfo, InstantiatorData)];

#[cfg(feature = "async")]
#[distributed_slice]
pub static __ASYNC_ENTRY_GETTERS: [fn() -> (TypeInfo, AsyncInstantiatorData)];
