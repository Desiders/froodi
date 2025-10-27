use alloc::collections::btree_map::BTreeMap;
use core::{future::Future, pin::Pin};

use crate::{
    any::TypeInfo,
    async_impl::{
        finalizer::boxed_finalizer_factory,
        instantiator::{boxed_instantiator, Instantiator},
        registry::InstantiatorData,
        Finalizer, Registry,
    },
    dependency_resolver::DependencyResolver,
    utils::{
        hlist,
        thread_safety::{SendSafety, SyncSafety},
    },
    Config, InstantiateErrorKind, ResolveErrorKind, Scope, Scopes,
};

#[inline]
#[must_use]
#[doc(hidden)]
pub fn build_registry<H, S, const N: usize>((_, scope_entries): (S, H)) -> Registry
where
    S: Scope + Scopes<N, Scope = S>,
    H: hlist::IntoIterator<(TypeInfo, InstantiatorData)>,
{
    let mut entries = BTreeMap::new();
    for (type_info, data) in scope_entries.into_iter() {
        entries.insert(type_info, data);
    }
    Registry::new::<S, S, N>(entries)
}

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
