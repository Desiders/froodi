#[cfg(feature = "async")]
use crate::async_impl::Instantiator as AsyncInstantiator;
#[cfg(feature = "async")]
use crate::registry::linking::AsyncExecution;
use crate::{
    errors::InstantiatorErrorKind,
    registry::{linking::SyncExecution, RegistrationId},
    utils::thread_safety::{RcThreadSafety, SendSafety, SyncSafety},
    Container, Dependency, DependencyResolver, Inject, InstantiateErrorKind, Instantiator, ResolveErrorKind,
};
use alloc::collections::BTreeSet;
#[cfg(feature = "async")]
use core::future::Future;
use core::marker::PhantomData;

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
/// Register the type with `provide::<T>(scope)` in `registry!`, or
/// `provide::<T>()` inside a scope block.
/// Scope, `config`, and `finalizer` are set in the registry.
///
/// `Execution` selects how dependencies are resolved before the factory assembles the fields.
pub trait Construct: Sized + 'static {
    type Deps: DependencyResolver;
    type Execution;

    fn instantiator() -> impl Instantiator<Self::Deps, Provides = Self, Error = InstantiateErrorKind> + SendSafety + SyncSafety;
}

pub struct ConstructInstantiator<Inst, Execution> {
    inst: Inst,
    marker: PhantomData<fn() -> Execution>,
}

impl<Inst: Clone, Execution> Clone for ConstructInstantiator<Inst, Execution> {
    fn clone(&self) -> Self {
        Self {
            inst: self.inst.clone(),
            marker: PhantomData,
        }
    }
}

impl<Inst: Instantiator<Deps>, Deps: DependencyResolver> Instantiator<Deps> for ConstructInstantiator<Inst, SyncExecution> {
    type Provides = Inst::Provides;
    type Error = Inst::Error;

    #[inline]
    fn instantiate(&mut self, dependencies: Deps) -> Result<Self::Provides, Self::Error> {
        self.inst.instantiate(dependencies)
    }

    fn dependencies() -> BTreeSet<Dependency> {
        Inst::dependencies()
    }

    #[inline]
    fn instantiate_linked(
        &self,
        container: &Container,
        edges: &[RegistrationId],
    ) -> Result<Self::Provides, InstantiatorErrorKind<ResolveErrorKind, InstantiateErrorKind>> {
        self.inst.instantiate_linked(container, edges)
    }
}

#[cfg(feature = "async")]
impl<Inst: Instantiator<Deps> + SendSafety, Deps: DependencyResolver + SendSafety> AsyncInstantiator<Deps>
    for ConstructInstantiator<Inst, AsyncExecution>
{
    type Provides = Inst::Provides;
    type Error = Inst::Error;

    #[inline]
    fn instantiate(&mut self, dependencies: Deps) -> impl Future<Output = Result<Self::Provides, Self::Error>> + SendSafety {
        async move { self.inst.instantiate(dependencies) }
    }

    fn dependencies() -> BTreeSet<Dependency> {
        Inst::dependencies()
    }
}

#[must_use]
#[inline]
pub fn provide<T: Construct>() -> ConstructInstantiator<impl Instantiator<T::Deps, Provides = T, Error = InstantiateErrorKind>, T::Execution>
{
    ConstructInstantiator {
        inst: T::instantiator(),
        marker: PhantomData,
    }
}
