use alloc::collections::BTreeMap;
use core::any::TypeId;

use crate::utils::thread_safety::RcAnyThreadSafety;

pub(crate) type Map = BTreeMap<TypeId, RcAnyThreadSafety>;
