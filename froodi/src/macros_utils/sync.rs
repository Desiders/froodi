use alloc::collections::btree_map::BTreeMap;
use core::marker::PhantomData;

use crate::{
    any::TypeInfo,
    dependency_resolver::DependencyResolver,
    finalizer::boxed_finalizer_factory,
    instantiator::{boxed_instantiator, Instantiator},
    macros_utils::types::RegistryOrEntry,
    registry::InstantiatorData,
    utils::thread_safety::{SendSafety, SyncSafety},
    Config, Finalizer, InstantiateErrorKind, Registry, ResolveErrorKind, Scope, Scopes,
};

/// Flat accumulator behind the `registry!` macro. Nothing about the shape of a registry is
/// encoded in a type, so neither macro-expansion nor trait-resolution depth grows with the
/// number of `provide(...)` items.
///
/// `S` is inferred from the scopes the clauses pass to [`RegistryBuilder::set_scope`], which is
/// what makes [`RegistryBuilder::build`] able to call the scope-typed `Registry::new`.
#[doc(hidden)]
pub struct RegistryBuilder<S, const N: usize> {
    entries: BTreeMap<TypeInfo, InstantiatorData>,
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
            scope: PhantomData,
        }
    }

    /// Anchors `S`, the scope type whose `Scopes::all()` fills `Registry::scopes_data`. Every
    /// clause calls this, so all of a registry's scopes must share one type.
    #[inline]
    #[doc(hidden)]
    pub fn set_scope(&mut self, _scope: S) {}

    #[inline]
    #[doc(hidden)]
    pub fn push(&mut self, registry_or_entry: RegistryOrEntry) {
        match registry_or_entry {
            RegistryOrEntry::Registry(registry) => {
                self.entries.extend(registry.entries);
            }
            RegistryOrEntry::Entry((key, value)) => {
                self.entries.insert(key, value);
            }
        }
    }

    #[inline]
    #[doc(hidden)]
    pub fn push_registry(&mut self, registry: Registry) {
        self.entries.extend(registry.entries);
    }

    #[inline]
    #[must_use]
    #[doc(hidden)]
    pub fn build(self) -> Registry {
        Registry::new::<S, S, N>(self.entries)
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

pub type FinDummy<T> = fn(T) -> ();
