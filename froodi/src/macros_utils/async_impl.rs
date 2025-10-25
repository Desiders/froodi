use alloc::{collections::btree_map::BTreeMap, vec::Vec};
use core::{any::TypeId, future::Future, pin::Pin};

use crate::{
    async_impl::{
        finalizer::boxed_finalizer_factory,
        instantiator::{boxed_instantiator, Instantiator},
        registry::InstantiatorData,
        Finalizer, Registry,
    },
    dependency_resolver::DependencyResolver,
    utils::thread_safety::{SendSafety, SyncSafety},
    Config, InstantiateErrorKind, ResolveErrorKind, Scope, Scopes,
};

#[inline]
#[doc(hidden)]
pub fn build_registry<S, const SCOPES_N: usize, const N: usize>(scopes_entries: [(S, Vec<(TypeId, InstantiatorData)>); N]) -> Registry
where
    S: Scope + Scopes<SCOPES_N, Scope = S>,
{
    let mut entries = BTreeMap::new();
    for (_, scope_entries) in scopes_entries {
        for (type_id, data) in scope_entries {
            entries.insert(type_id, data);
        }
    }
    Registry::new::<S, S, SCOPES_N>(entries)
}

#[inline]
#[doc(hidden)]
pub fn make_entry<Inst, Deps, Fin>(scope: impl Scope, inst: Inst, config: Option<Config>, fin: Option<Fin>) -> (TypeId, InstantiatorData)
where
    Inst: Instantiator<Deps, Error = InstantiateErrorKind> + SendSafety + SyncSafety,
    Inst::Provides: SendSafety + SyncSafety,
    Deps: DependencyResolver<Error = ResolveErrorKind>,
    Fin: Finalizer<Inst::Provides> + SendSafety + SyncSafety,
{
    (
        TypeId::of::<Inst::Provides>(),
        InstantiatorData {
            dependencies: Inst::dependencies(),
            instantiator: boxed_instantiator(inst),
            finalizer: fin.map(boxed_finalizer_factory),
            config: config.unwrap_or_default(),
            scope_data: scope.into(),
        },
    )
}

#[cfg(feature = "thread_safe")]
pub type FinDummy<T> = fn(T) -> Pin<super::aliases::Box<dyn Future<Output = ()> + Send>>;
#[cfg(not(feature = "thread_safe"))]
pub type FinDummy<T> = fn(T) -> Pin<super::aliases::Box<dyn Future<Output = ()>>>;
