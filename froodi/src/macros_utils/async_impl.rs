use alloc::boxed::Box;
use core::{future::Future, pin::Pin};

#[cfg(all(
    feature = "std",
    feature = "thread_safe",
    not(feature = "lock-parking-lot"),
    not(feature = "lock-spin")
))]
use crate::registry::RegistrationId;
use crate::{
    any::TypeInfo,
    async_impl::{
        finalizer::boxed_finalizer_factory,
        instantiator::{boxed_instantiator, Instantiator},
        Finalizer,
    },
    dependency_resolver::DependencyResolver,
    registry::{AsyncInstantiatorData as InstantiatorData, RegistrationMetadata},
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
            instantiator: boxed_instantiator(inst).into(),
            finalizer: fin.map(boxed_finalizer_factory),
            metadata: RegistrationMetadata {
                #[cfg(all(
                    feature = "std",
                    feature = "thread_safe",
                    not(feature = "lock-parking-lot"),
                    not(feature = "lock-spin")
                ))]
                lock_id: RegistrationId::UNINDEXED_LOCK,
                dependencies: Inst::dependencies(),
                config: config.unwrap_or_default(),
                scope_data: scope.into(),
            },
        },
    )
}

#[cfg(feature = "thread_safe")]
pub type FinDummy<T> = fn(T) -> Pin<Box<dyn Future<Output = ()> + Send>>;

#[cfg(not(feature = "thread_safe"))]
pub type FinDummy<T> = fn(T) -> Pin<Box<dyn Future<Output = ()>>>;
