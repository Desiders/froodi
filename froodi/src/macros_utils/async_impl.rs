use alloc::collections::btree_map::BTreeMap;
use core::{future::Future, pin::Pin};

use crate::{
    any::TypeInfo,
    async_impl::{
        self,
        finalizer::boxed_finalizer_factory,
        instantiator::{boxed_instantiator, Instantiator},
        registry::InstantiatorData,
        Finalizer, RegistryWithSync,
    },
    dependency_resolver::DependencyResolver,
    macros_utils::types::{RegistryKind, RegistryKindOrEntry},
    utils::{
        hlist,
        thread_safety::{SendSafety, SyncSafety},
    },
    Config, InstantiateErrorKind, Registry, ResolveErrorKind, Scope, Scopes,
};

#[inline]
#[must_use]
#[doc(hidden)]
pub fn build_registry<H, S, const N: usize>((_, iterable): (S, H)) -> RegistryWithSync
where
    S: Scope + Scopes<N, Scope = S>,
    H: hlist::IntoIterator<RegistryKindOrEntry>,
{
    let mut entries = BTreeMap::new();
    let mut sync_entries = BTreeMap::new();
    for registry_kind_or_entry in iterable.into_iter() {
        match registry_kind_or_entry {
            RegistryKindOrEntry::Kind(RegistryKind::Sync(registry)) => {
                sync_entries.extend(registry.entries);
            }
            RegistryKindOrEntry::Kind(RegistryKind::Async(registry)) => {
                entries.extend(registry.entries);
            }
            RegistryKindOrEntry::Kind(RegistryKind::AsyncWithSync(RegistryWithSync { registry, sync })) => {
                entries.extend(registry.entries);
                sync_entries.extend(sync.entries);
            }
            RegistryKindOrEntry::Entry((key, value)) => {
                entries.insert(key, value);
            }
        }
    }
    RegistryWithSync {
        registry: async_impl::Registry::new::<S, S, N>(entries),
        sync: Registry::new::<S, S, N>(sync_entries),
    }
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
