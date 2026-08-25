use alloc::collections::btree_map::BTreeMap;
use core::{future::Future, marker::PhantomData, pin::Pin};

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
    registry::InstantiatorData as SyncInstantiatorData,
    utils::thread_safety::{SendSafety, SyncSafety},
    Config, InstantiateErrorKind, Registry, ResolveErrorKind, Scope, Scopes,
};

type AsyncEntries = BTreeMap<TypeInfo, InstantiatorData>;
type SyncEntries = BTreeMap<TypeInfo, SyncInstantiatorData>;

/// Flat accumulator behind the `async_registry!` macro; see
/// [`crate::macros_utils::sync::RegistryBuilder`].
#[doc(hidden)]
pub struct RegistryBuilder<S, const N: usize> {
    entries: AsyncEntries,
    sync_entries: SyncEntries,
    scope: PhantomData<S>,
}

impl<S, const N: usize> Default for RegistryBuilder<S, N>
where
    S: Scope + Scopes<N, Scope = S>,
{
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<S, const N: usize> RegistryBuilder<S, N>
where
    S: Scope + Scopes<N, Scope = S>,
{
    #[inline]
    #[must_use]
    #[doc(hidden)]
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            sync_entries: BTreeMap::new(),
            scope: PhantomData,
        }
    }

    /// Anchors `S`, the scope type whose `Scopes::all()` fills `scopes_data`. Every clause calls
    /// this, so all of a registry's scopes must share one type.
    #[inline]
    #[doc(hidden)]
    pub fn set_scope(&mut self, _scope: S) {}

    #[inline]
    #[doc(hidden)]
    pub fn push(&mut self, registry_kind_or_entry: RegistryKindOrEntry) {
        match registry_kind_or_entry {
            RegistryKindOrEntry::Kind(kind) => self.push_kind(kind),
            RegistryKindOrEntry::Entry((key, value)) => {
                self.entries.insert(key, value);
            }
        }
    }

    #[inline]
    fn push_kind(&mut self, kind: RegistryKind) {
        match kind {
            RegistryKind::Sync(registry) => self.sync_entries.extend(registry.entries),
            RegistryKind::Async(registry) => self.entries.extend(registry.entries),
            RegistryKind::AsyncWithSync(RegistryWithSync { registry, sync }) => {
                self.entries.extend(registry.entries);
                self.sync_entries.extend(sync.entries);
            }
        }
    }

    #[inline]
    #[doc(hidden)]
    pub fn push_registry<R>(&mut self, registry: R)
    where
        R: IntoRegistryKind,
    {
        self.push_kind(registry.into_registry_kind());
    }

    #[inline]
    #[must_use]
    #[doc(hidden)]
    pub fn build(self) -> RegistryWithSync {
        RegistryWithSync {
            registry: async_impl::Registry::new::<S, S, N>(self.entries),
            sync: Registry::new::<S, S, N>(self.sync_entries),
        }
    }
}

/// Lets `extend(...)` take a sync, an async or a mixed registry without the macro knowing which.
#[doc(hidden)]
pub trait IntoRegistryKind {
    fn into_registry_kind(self) -> RegistryKind;
}

impl IntoRegistryKind for Registry {
    #[inline]
    fn into_registry_kind(self) -> RegistryKind {
        RegistryKind::Sync(self)
    }
}

impl IntoRegistryKind for async_impl::Registry {
    #[inline]
    fn into_registry_kind(self) -> RegistryKind {
        RegistryKind::Async(self)
    }
}

impl IntoRegistryKind for RegistryWithSync {
    #[inline]
    fn into_registry_kind(self) -> RegistryKind {
        RegistryKind::AsyncWithSync(self)
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
