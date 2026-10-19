use crate::{
    any::TypeInfo,
    dependency_resolver::DependencyResolver,
    finalizer::boxed_finalizer_factory,
    instantiator::{boxed_instantiator, Instantiator},
    registry::{InstantiatorData, RegistrationMetadata},
    utils::thread_safety::{SendSafety, SyncSafety},
    Config, Finalizer, InstantiateErrorKind, ResolveErrorKind, Scope,
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
                dependencies: Inst::dependencies(),
                config: config.unwrap_or_default(),
                scope_data: scope.into(),
            },
        },
    )
}

pub type FinDummy<T> = fn(T) -> ();
