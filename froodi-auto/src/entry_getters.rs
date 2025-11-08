#[cfg(feature = "async")]
use froodi::async_impl;
use froodi::{InstantiatorData, TypeInfo};

pub use linkme::{self, distributed_slice};

#[distributed_slice]
pub static __ENTRY_GETTERS: [fn() -> (TypeInfo, InstantiatorData)];

#[cfg(feature = "async")]
#[distributed_slice]
pub static __ASYNC_ENTRY_GETTERS: [fn() -> (TypeInfo, async_impl::InstantiatorData)];
