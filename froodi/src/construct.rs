use crate::{
    utils::thread_safety::{RcThreadSafety, SendSafety, SyncSafety},
    DependencyResolver, Inject, InstantiateErrorKind, Instantiator,
};

mod sealed {
    use crate::utils::thread_safety::RcThreadSafety;

    pub trait Sealed {}

    impl<T: ?Sized> Sealed for RcThreadSafety<T> {}
}

#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "Construct field `{Self}` cannot use Inject resolution",
    note = "Inject fields use Arc<T> in thread-safe mode or Rc<T> in local mode; select #[di(inject_transient)] for InjectTransient resolution, or write provide(...)"
)]
pub trait ConstructField<Dep>: sealed::Sealed {
    fn from_inject(value: Inject<Dep>) -> Self;
}

impl<T> ConstructField<T> for RcThreadSafety<T> {
    fn from_inject(value: Inject<T>) -> Self {
        value.0
    }
}

impl<T: ?Sized> ConstructField<RcThreadSafety<T>> for RcThreadSafety<T> {
    fn from_inject(value: Inject<RcThreadSafety<T>>) -> Self {
        (*value.0).clone()
    }
}

/// Builds a type from its fields.
///
/// Fields use [`Inject`] by default; `#[di(inject)]` selects it explicitly.
/// Use `#[di(inject_transient)]` for [`InjectTransient`](crate::InjectTransient).
///
/// Register the type with `construct::<T>()` in a `registry!` scope block.
/// Scope, `config`, and `finalizer` are set in the registry.
pub trait Construct: Sized + 'static {
    type Deps: DependencyResolver;

    fn instantiator() -> impl Instantiator<Self::Deps, Provides = Self, Error = InstantiateErrorKind> + SendSafety + SyncSafety;
}

/// Adapts a [`Construct`] implementation to an ordinary synchronous registration.
#[must_use]
pub fn construct<T: Construct>() -> impl Instantiator<T::Deps, Provides = T, Error = InstantiateErrorKind> + SendSafety + SyncSafety {
    T::instantiator()
}
