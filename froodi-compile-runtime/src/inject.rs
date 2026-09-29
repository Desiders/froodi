use crate::thread_safety::RcThreadSafety;

/// A `get`-like dependency: shared, scoped, subject to the provider's cache policy and finalizer.
pub struct Inject<Dep>(pub RcThreadSafety<Dep>);
