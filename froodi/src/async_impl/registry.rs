use alloc::{collections::BTreeMap, vec::Vec};
use core::any::TypeId;

use crate::{
    async_impl::{
        finalizer::{boxed_finalizer_factory as boxed_async_finalizer_factory, BoxedCloneFinalizer, Finalizer as AsyncFinalizer},
        instantiator::{
            boxed_container_instantiator, boxed_instantiator as boxed_async_instantiator_factory,
            BoxedCloneInstantiator as BoxedCloneAsyncInstantiator, Instantiator as AsyncInstantiator,
        },
        Container,
    },
    dependency_resolver::DependencyResolver,
    errors::{InstantiateErrorKind, ResolveErrorKind},
    finalizer::{boxed_finalizer_factory, BoxedCloneFinalizer as BoxedCloneSyncFinalizer, Finalizer},
    instantiator::{
        boxed_container_instantiator as boxed_sync_container_instantiator, boxed_instantiator, BoxedCloneInstantiator, Instantiator,
    },
    registry::{InstantiatorData as SyncInstantiatorData, Registry as SyncRegistry},
    scope::{Scope, ScopeData},
    utils::thread_safety::{SendSafety, SyncSafety},
    Config, DefaultScope, Scopes as ScopesTrait,
};

#[derive(Clone)]
pub(crate) enum BoxedInstantiator {
    Sync(BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>),
    Async(BoxedCloneAsyncInstantiator<ResolveErrorKind, InstantiateErrorKind>),
}

#[derive(Clone)]
pub(crate) struct InstantiatorData {
    pub(crate) instantiator: BoxedCloneAsyncInstantiator<ResolveErrorKind, InstantiateErrorKind>,
    pub(crate) finalizer: Option<BoxedCloneFinalizer>,
    pub(crate) config: Config,
    pub(crate) scope_data: ScopeData,
}

#[derive(Clone)]
pub(crate) struct BoxedInstantiatorData {
    instantiator: BoxedInstantiator,
    config: Config,
    scope_data: ScopeData,
}

pub struct RegistryBuilder<S> {
    instantiators: BTreeMap<TypeId, BoxedInstantiatorData>,
    finalizers: BTreeMap<TypeId, BoxedCloneFinalizer>,
    sync_finalizers: BTreeMap<TypeId, BoxedCloneSyncFinalizer>,
    scopes: Vec<S>,
}

impl Default for RegistryBuilder<DefaultScope> {
    fn default() -> Self {
        Self::new()
    }
}

impl RegistryBuilder<DefaultScope> {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            instantiators: BTreeMap::new(),
            finalizers: BTreeMap::new(),
            sync_finalizers: BTreeMap::new(),
            scopes: Vec::from(DefaultScope::all()),
        }
    }
}

impl<S> RegistryBuilder<S> {
    #[inline]
    #[must_use]
    pub fn new_with_scopes<Scopes, const N: usize>() -> Self
    where
        Scopes: ScopesTrait<N, Scope = S>,
    {
        Self {
            instantiators: BTreeMap::new(),
            finalizers: BTreeMap::new(),
            sync_finalizers: BTreeMap::new(),
            scopes: Vec::from(Scopes::all()),
        }
    }
}

impl<S> RegistryBuilder<S> {
    #[inline]
    #[must_use]
    pub fn provide<Inst, Deps>(mut self, instantiator: Inst, scope: S) -> Self
    where
        Inst: Instantiator<Deps, Error = InstantiateErrorKind> + SendSafety + SyncSafety,
        Deps: DependencyResolver<Error = ResolveErrorKind>,
        S: Scope,
    {
        self.add_instantiator::<Inst::Provides>(BoxedInstantiator::Sync(boxed_instantiator(instantiator)), scope);
        self
    }

    #[inline]
    #[must_use]
    pub fn provide_with_config<Inst, Deps>(mut self, instantiator: Inst, config: Config, scope: S) -> Self
    where
        Inst: Instantiator<Deps, Error = InstantiateErrorKind> + SendSafety + SyncSafety,
        Deps: DependencyResolver<Error = ResolveErrorKind>,
        S: Scope,
    {
        self.add_instantiator_with_config::<Inst::Provides>(BoxedInstantiator::Sync(boxed_instantiator(instantiator)), config, scope);
        self
    }

    #[inline]
    #[must_use]
    pub fn provide_async<Inst, Deps>(mut self, instantiator: Inst, scope: S) -> Self
    where
        Inst: AsyncInstantiator<Deps, Error = InstantiateErrorKind> + SendSafety + SyncSafety,
        Deps: DependencyResolver<Error = ResolveErrorKind>,
        S: Scope,
    {
        self.add_instantiator::<Inst::Provides>(BoxedInstantiator::Async(boxed_async_instantiator_factory(instantiator)), scope);
        self
    }

    #[inline]
    #[must_use]
    pub fn provide_async_with_config<Inst, Deps>(mut self, instantiator: Inst, config: Config, scope: S) -> Self
    where
        Inst: AsyncInstantiator<Deps, Error = InstantiateErrorKind> + SendSafety + SyncSafety,
        Deps: DependencyResolver<Error = ResolveErrorKind>,
        S: Scope,
    {
        self.add_instantiator_with_config::<Inst::Provides>(
            BoxedInstantiator::Async(boxed_async_instantiator_factory(instantiator)),
            config,
            scope,
        );
        self
    }

    /// Adds a finalizer for the given a non transient dependency type.
    /// The finalizer will be called when the container is being closed in LIFO order of their usage (not the order of registration).
    ///
    /// # Notes
    /// Calls to async finalizers precede calls to sync finalizers
    ///
    /// # Warning
    /// - The finalizer can only be used for non-transient dependencies, because the transient doesn't have a lifetime and isn't cached.
    ///
    /// - [`Drop`] trait isn't a equivalent of a finalizer, because:
    ///     1. The finalizer is called in LIFO order of their usage, while [`Drop`] is called in the order of registration.
    ///     2. The finalized used for life cycle management, while [`Drop`] is used for resource management.
    #[inline]
    #[must_use]
    pub fn add_finalizer<Dep: SendSafety + SyncSafety + 'static>(
        mut self,
        finalizer: impl Finalizer<Dep> + SendSafety + SyncSafety,
    ) -> Self {
        self.sync_finalizers.insert(TypeId::of::<Dep>(), boxed_finalizer_factory(finalizer));
        self
    }

    /// Adds an async finalizer for the given a non transient dependency type.
    /// The finalizer will be called when the container is being closed in LIFO order of their usage (not the order of registration).
    ///
    /// # Notes
    /// Calls to async finalizers precede calls to sync finalizers
    ///
    /// # Warning
    /// - The finalizer can only be used for non-transient dependencies, because the transient doesn't have a lifetime and isn't cached.
    ///
    /// - [`Drop`] trait isn't a equivalent of a finalizer, because:
    ///     1. The finalizer is called in LIFO order of their usage, while [`Drop`] is called in the order of registration.
    ///     2. The finalized used for life cycle management, while [`Drop`] is used for resource management.
    #[inline]
    #[must_use]
    pub fn add_async_finalizer<Dep: SendSafety + SyncSafety + 'static>(
        mut self,
        finalizer: impl AsyncFinalizer<Dep> + SendSafety + SyncSafety,
    ) -> Self {
        self.finalizers
            .insert(TypeId::of::<Dep>(), boxed_async_finalizer_factory(finalizer));
        self
    }
}

impl<S> RegistryBuilder<S>
where
    S: Scope,
{
    #[inline]
    pub(crate) fn add_instantiator<Dep: 'static>(&mut self, instantiator: BoxedInstantiator, scope: S) -> Option<BoxedInstantiatorData> {
        self.add_instantiator_with_config::<Dep>(instantiator, Config::default(), scope)
    }

    #[inline]
    pub(crate) fn add_instantiator_with_config<Dep: 'static>(
        &mut self,
        instantiator: BoxedInstantiator,
        config: Config,
        scope: S,
    ) -> Option<BoxedInstantiatorData> {
        self.instantiators.insert(
            TypeId::of::<Dep>(),
            BoxedInstantiatorData {
                instantiator,
                config,
                scope_data: scope.into(),
            },
        )
    }
}

pub(crate) struct RegistryWithScopes {
    pub(crate) registry: Registry,
    pub(crate) sync_registry: SyncRegistry,
    pub(crate) scope_data: ScopeData,
    pub(crate) child_scopes_data: Vec<ScopeData>,
}

impl<S> RegistryBuilder<S>
where
    S: Scope + Clone,
{
    pub(crate) fn build(mut self) -> RegistryWithScopes {
        let mut scope_iter = self.scopes.into_iter();
        let scope_data = scope_iter.next().expect("registries len (is 0) should be > 0").into();
        let child_scopes_data = scope_iter.map(Into::into).collect();

        let mut instantiators = BTreeMap::new();
        let mut sync_instantiators = BTreeMap::new();
        for (
            type_id,
            BoxedInstantiatorData {
                instantiator,
                config,
                scope_data,
            },
        ) in self.instantiators
        {
            match instantiator {
                BoxedInstantiator::Sync(instantiator) => {
                    sync_instantiators.insert(
                        type_id,
                        SyncInstantiatorData {
                            instantiator,
                            finalizer: self.sync_finalizers.remove(&type_id),
                            config,
                            scope_data,
                        },
                    );
                }
                BoxedInstantiator::Async(instantiator) => {
                    instantiators.insert(
                        type_id,
                        InstantiatorData {
                            instantiator,
                            finalizer: self.finalizers.remove(&type_id),
                            config,
                            scope_data,
                        },
                    );
                }
            }
        }

        let registry = Registry {
            instantiators: instantiators
                .into_iter()
                .chain(Some((
                    TypeId::of::<Container>(),
                    InstantiatorData {
                        instantiator: boxed_container_instantiator(),
                        finalizer: None,
                        config: Config { cache_provides: true },
                        scope_data,
                    },
                )))
                .collect(),
        };
        let sync_registry = SyncRegistry {
            instantiators: sync_instantiators
                .into_iter()
                .chain(Some((
                    TypeId::of::<Container>(),
                    SyncInstantiatorData {
                        instantiator: boxed_sync_container_instantiator(),
                        finalizer: None,
                        config: Config { cache_provides: true },
                        scope_data,
                    },
                )))
                .collect(),
        };

        RegistryWithScopes {
            registry,
            sync_registry,
            scope_data,
            child_scopes_data,
        }
    }
}

pub(crate) struct Registry {
    pub(crate) instantiators: BTreeMap<TypeId, InstantiatorData>,
}

impl Registry {
    #[inline]
    pub(crate) fn get_instantiator_data(&self, type_id: &TypeId) -> Option<&InstantiatorData> {
        self.instantiators.get(type_id)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BoxedCloneInstantiator, InstantiateErrorKind, Registry, RegistryBuilder, RegistryWithScopes, ResolveErrorKind, SyncRegistry,
    };
    use crate::{
        scope::DefaultScope::{self, *},
        utils::thread_safety::RcThreadSafety,
        Scopes,
    };
    use core::any::TypeId;

    #[test]
    fn test_build_empty() {
        let RegistryWithScopes {
            registry: Registry { instantiators },
            sync_registry: SyncRegistry {
                instantiators: sync_instantiators,
            },
            child_scopes_data,
            ..
        } = RegistryBuilder::<DefaultScope>::new().build();
        assert_eq!(instantiators.len(), 1);
        assert_eq!(sync_instantiators.len(), 1);
        assert!(!child_scopes_data.is_empty());
    }

    #[test]
    fn test_build_equal_provides() {
        let RegistryWithScopes {
            registry,
            sync_registry,
            child_scopes_data,
            ..
        } = RegistryBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(()), App)
            .provide(|| Ok(()), App)
            .provide_async(async || Ok(((), ())), Runtime)
            .provide_async(async || Ok(((), ())), Runtime)
            .provide_async(async || Ok(((), ())), App)
            .provide_async(async || Ok(((), ())), App)
            .build();
        assert_eq!(child_scopes_data.len() + 1, DefaultScope::all().len());
        assert_eq!(registry.instantiators.len(), 2);
        assert_eq!(sync_registry.instantiators.len(), 2);
    }

    #[test]
    fn test_build_several_scopes() {
        let RegistryWithScopes {
            registry,
            sync_registry,
            child_scopes_data,
            ..
        } = RegistryBuilder::new()
            .provide(|| Ok(1i8), Runtime)
            .provide(|| Ok(1i16), Runtime)
            .provide(|| Ok(1i32), App)
            .provide(|| Ok(1i64), App)
            .provide_async(async || Ok(1u8), Runtime)
            .provide_async(async || Ok(1u16), Runtime)
            .provide_async(async || Ok(1u32), App)
            .provide_async(async || Ok(1u64), App)
            .build();
        assert_eq!(child_scopes_data.len() + 1, DefaultScope::all().len());
        assert_eq!(registry.instantiators.len(), 5);
        assert_eq!(sync_registry.instantiators.len(), 5);
    }

    #[test]
    fn test_add_finalizer() {
        let RegistryWithScopes {
            registry: Registry { instantiators },
            sync_registry: SyncRegistry {
                instantiators: sync_instantiators,
            },
            ..
        } = RegistryBuilder::new()
            .provide(|| Ok(1i8), Runtime)
            .provide(|| Ok(1i16), Runtime)
            .provide(|| Ok(1i32), App)
            .provide(|| Ok(1i64), App)
            .provide_async(async || Ok(1u8), Runtime)
            .provide_async(async || Ok(1u16), Runtime)
            .provide_async(async || Ok(1u32), App)
            .provide_async(async || Ok(1u64), App)
            .add_finalizer(|_: RcThreadSafety<i8>| {})
            .add_finalizer(|_: RcThreadSafety<i32>| {})
            .add_async_finalizer(async |_: RcThreadSafety<u8>| {})
            .add_async_finalizer(async |_: RcThreadSafety<u32>| {})
            .build();

        let i8_type_id = TypeId::of::<i8>();
        let i16_type_id = TypeId::of::<i16>();
        let i32_type_id = TypeId::of::<i32>();
        let i64_type_id = TypeId::of::<i64>();

        if let Some(data) = instantiators.get(&i8_type_id) {
            assert!(data.finalizer.is_some());
        }
        if let Some(data) = instantiators.get(&i16_type_id) {
            assert!(data.finalizer.is_none());
        }
        if let Some(data) = instantiators.get(&i32_type_id) {
            assert!(data.finalizer.is_some());
        }
        if let Some(data) = instantiators.get(&i64_type_id) {
            assert!(data.finalizer.is_none());
        }
        if let Some(data) = sync_instantiators.get(&i8_type_id) {
            assert!(data.finalizer.is_some());
        }
        if let Some(data) = sync_instantiators.get(&i16_type_id) {
            assert!(data.finalizer.is_none());
        }
        if let Some(data) = sync_instantiators.get(&i32_type_id) {
            assert!(data.finalizer.is_some());
        }
        if let Some(data) = sync_instantiators.get(&i64_type_id) {
            assert!(data.finalizer.is_none());
        }
    }

    #[test]
    fn test_bounds() {
        fn impl_bounds<T: Send>() {}

        impl_bounds::<(BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,)>();
    }
}
