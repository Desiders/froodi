use alloc::boxed::Box;
use core::{future::Future, pin::Pin};

use crate::{
    any::TypeInfo,
    async_impl::{
        finalizer::boxed_finalizer_factory,
        instantiator::{boxed_instantiator, Instantiator},
        registry::InstantiatorData,
        Finalizer,
    },
    dependency_resolver::DependencyResolver,
    utils::thread_safety::{SendSafety, SyncSafety},
    Config, InstantiateErrorKind, ResolveErrorKind, Scope,
};

#[inline]
#[must_use]
#[doc(hidden)]
pub fn make_entry<Inst, Deps, Fin>(scope: impl Scope, inst: Inst, config: Option<Config>, fin: Option<Fin>) -> (TypeInfo, InstantiatorData)
where
    Inst: Instantiator<Deps, Error = InstantiateErrorKind> + SendSafety + SyncSafety,
    Inst::Provides: SendSafety + SyncSafety,
    Deps: DependencyResolver<Error = ResolveErrorKind>,
    Fin: Finalizer<Inst::Provides> + SendSafety + SyncSafety,
{
    (
        TypeInfo::of::<Inst::Provides>(),
        InstantiatorData {
            dependencies: Inst::dependencies(),
            instantiator: boxed_instantiator(inst).into(),
            finalizer: fin.map(boxed_finalizer_factory),
            config: config.unwrap_or_default(),
            scope_data: scope.into(),
        },
    )
}

#[cfg(feature = "thread_safe")]
pub type FinDummy<T> = fn(T) -> Pin<Box<dyn Future<Output = ()> + Send>>;

#[cfg(not(feature = "thread_safe"))]
pub type FinDummy<T> = fn(T) -> Pin<Box<dyn Future<Output = ()>>>;
