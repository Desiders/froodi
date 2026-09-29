use crate::thread_safety::RcThreadSafety;

/// A `get`-like dependency: scoped, subject to the provider's cache policy and finalizer.
pub struct Inject<Dep>(pub RcThreadSafety<Dep>);

/// A `get_transient`-like dependency: a fresh value, the provided-value cache is not used.
pub struct InjectTransient<Dep>(pub Dep);
