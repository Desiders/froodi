use alloc::{collections::BTreeMap, vec, vec::Vec};
use core::any::TypeId;

use crate::{
    dependency_resolver::DependencyResolver,
    errors::{InstantiateErrorKind, ResolveErrorKind},
    finalizer::{boxed_finalizer_factory, BoxedCloneFinalizer as BoxedCloneSyncFinalizer, Finalizer},
    instantiator::{boxed_instantiator_factory, BoxedCloneInstantiator, Config, Instantiator},
    r#async::{
        dependency_resolver::DependencyResolver as AsyncDependencyResolver,
        finalizer::{boxed_finalizer_factory as boxed_async_finalizer_factory, BoxedCloneFinalizer, Finalizer as AsyncFinalizer},
        instantiator::{
            boxed_instantiator_factory as boxed_async_instantiator_factory, BoxedCloneInstantiator as BoxedCloneAsyncInstantiator,
            Instantiator as AsyncInstantiator,
        },
    },
    registry::{InstantiatorInnerData as SyncInstantiatorInnerData, Registry as SyncRegistry},
    scope::{Scope, ScopeInnerData},
    DefaultScope, Scopes as ScopesTrait,
};

#[derive(Clone)]
pub(crate) enum BoxedInstantiator {
    Sync(BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>),
    Async(BoxedCloneAsyncInstantiator<ResolveErrorKind, InstantiateErrorKind>),
}

pub(crate) struct InstantiatorData<S> {
    instantiator: BoxedInstantiator,
    config: Config,
    scope: S,
}

pub struct RegistriesBuilder<Scope> {
    instantiators: BTreeMap<TypeId, InstantiatorData<Scope>>,
    finalizers: BTreeMap<TypeId, BoxedCloneFinalizer>,
    sync_finalizers: BTreeMap<TypeId, BoxedCloneSyncFinalizer>,
    scopes: Vec<Scope>,
}

impl Default for RegistriesBuilder<DefaultScope> {
    fn default() -> Self {
        Self::new()
    }
}

impl RegistriesBuilder<DefaultScope> {
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

impl<Scope> RegistriesBuilder<Scope> {
    #[inline]
    #[must_use]
    pub fn new_with_scopes<Scopes, const N: usize>() -> Self
    where
        Scopes: ScopesTrait<N, Scope = Scope>,
    {
        Self {
            instantiators: BTreeMap::new(),
            finalizers: BTreeMap::new(),
            sync_finalizers: BTreeMap::new(),
            scopes: Vec::from(Scopes::all()),
        }
    }
}

impl<S> RegistriesBuilder<S> {
    #[inline]
    #[must_use]
    pub fn provide<Inst, Deps>(mut self, instantiator: Inst, scope: S) -> Self
    where
        Inst: Instantiator<Deps, Error = InstantiateErrorKind> + Send + Sync,
        Deps: DependencyResolver<Error = ResolveErrorKind>,
    {
        self.add_instantiator::<Inst::Provides>(BoxedInstantiator::Sync(boxed_instantiator_factory(instantiator)), scope);
        self
    }

    #[inline]
    #[must_use]
    pub fn provide_with_config<Inst, Deps>(mut self, instantiator: Inst, config: Config, scope: S) -> Self
    where
        Inst: Instantiator<Deps, Error = InstantiateErrorKind> + Send + Sync,
        Deps: DependencyResolver<Error = ResolveErrorKind>,
    {
        self.add_instantiator_with_config::<Inst::Provides>(
            BoxedInstantiator::Sync(boxed_instantiator_factory(instantiator)),
            config,
            scope,
        );
        self
    }

    #[inline]
    #[must_use]
    pub fn provide_async<Inst, Deps>(mut self, instantiator: Inst, scope: S) -> Self
    where
        Inst: AsyncInstantiator<Deps, Provides: Send, Error = InstantiateErrorKind> + Send + Sync,
        Deps: AsyncDependencyResolver<Error = ResolveErrorKind>,
    {
        self.add_instantiator::<Inst::Provides>(BoxedInstantiator::Async(boxed_async_instantiator_factory(instantiator)), scope);
        self
    }

    #[inline]
    #[must_use]
    pub fn provide_async_with_config<Inst, Deps>(mut self, instantiator: Inst, config: Config, scope: S) -> Self
    where
        Inst: AsyncInstantiator<Deps, Provides: Send, Error = InstantiateErrorKind> + Send + Sync,
        Deps: AsyncDependencyResolver<Error = ResolveErrorKind>,
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
    pub fn add_finalizer<Dep, Fin>(mut self, finalizer: Fin) -> Self
    where
        Dep: Send + Sync + 'static,
        Fin: Finalizer<Dep> + Send + Sync,
    {
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
    pub fn add_async_finalizer<Dep, Fin>(mut self, finalizer: Fin) -> Self
    where
        Dep: Send + Sync + 'static,
        Fin: AsyncFinalizer<Dep> + Send + Sync,
    {
        self.finalizers
            .insert(TypeId::of::<Dep>(), boxed_async_finalizer_factory(finalizer));
        self
    }
}

impl<S> RegistriesBuilder<S> {
    #[inline]
    pub(crate) fn add_instantiator<Dep: 'static>(&mut self, instantiator: BoxedInstantiator, scope: S) -> Option<InstantiatorData<S>> {
        self.add_instantiator_with_config::<Dep>(instantiator, Config::default(), scope)
    }

    #[inline]
    pub(crate) fn add_instantiator_with_config<Dep: 'static>(
        &mut self,
        instantiator: BoxedInstantiator,
        config: Config,
        scope: S,
    ) -> Option<InstantiatorData<S>> {
        self.instantiators.insert(
            TypeId::of::<Dep>(),
            InstantiatorData {
                instantiator,
                config,
                scope,
            },
        )
    }
}

impl<S> RegistriesBuilder<S>
where
    S: Scope + Clone,
{
    pub(crate) fn build(mut self) -> (Vec<Registry>, Vec<SyncRegistry>) {
        use alloc::collections::btree_map::Entry::{Occupied, Vacant};

        let mut scopes_instantiators: BTreeMap<S, Vec<(TypeId, InstantiatorInnerData)>> =
            self.scopes.clone().into_iter().map(|scope| (scope, Vec::new())).collect();
        let mut scopes_sync_instantiators: BTreeMap<S, Vec<(TypeId, SyncInstantiatorInnerData)>> =
            self.scopes.into_iter().map(|scope| (scope, Vec::new())).collect();
        for (
            type_id,
            InstantiatorData {
                instantiator,
                config,
                scope,
            },
        ) in self.instantiators
        {
            match instantiator {
                BoxedInstantiator::Sync(instantiator) => match scopes_sync_instantiators.entry(scope) {
                    Vacant(entry) => {
                        entry.insert(vec![(
                            type_id,
                            SyncInstantiatorInnerData {
                                instantiator,
                                finalizer: self.sync_finalizers.remove(&type_id),
                                config,
                            },
                        )]);
                    }
                    Occupied(entry) => {
                        entry.into_mut().push((
                            type_id,
                            SyncInstantiatorInnerData {
                                instantiator,
                                finalizer: self.sync_finalizers.remove(&type_id),
                                config,
                            },
                        ));
                    }
                },
                BoxedInstantiator::Async(instantiator) => match scopes_instantiators.entry(scope) {
                    Vacant(entry) => {
                        entry.insert(vec![(
                            type_id,
                            InstantiatorInnerData {
                                instantiator,
                                finalizer: self.finalizers.remove(&type_id),
                                config,
                            },
                        )]);
                    }
                    Occupied(entry) => {
                        entry.into_mut().push((
                            type_id,
                            InstantiatorInnerData {
                                instantiator,
                                finalizer: self.finalizers.remove(&type_id),
                                config,
                            },
                        ));
                    }
                },
            }
        }

        let mut registries = Vec::with_capacity(scopes_instantiators.len());
        for (scope, instantiators) in scopes_instantiators {
            registries.push(Registry {
                scope: ScopeInnerData {
                    priority: scope.priority(),
                    is_skipped_by_default: scope.is_skipped_by_default(),
                },
                instantiators: BTreeMap::from_iter(instantiators),
            });
        }

        let mut sync_registries = Vec::with_capacity(scopes_sync_instantiators.len());
        for (scope, instantiators) in scopes_sync_instantiators {
            sync_registries.push(SyncRegistry {
                scope: ScopeInnerData {
                    priority: scope.priority(),
                    is_skipped_by_default: scope.is_skipped_by_default(),
                },
                instantiators: BTreeMap::from_iter(instantiators),
            });
        }

        (registries, sync_registries)
    }
}

#[derive(Clone)]
pub(crate) struct InstantiatorInnerData {
    pub(crate) instantiator: BoxedCloneAsyncInstantiator<ResolveErrorKind, InstantiateErrorKind>,
    pub(crate) finalizer: Option<BoxedCloneFinalizer>,
    pub(crate) config: Config,
}

pub(crate) struct Registry {
    pub(crate) scope: ScopeInnerData,
    instantiators: BTreeMap<TypeId, InstantiatorInnerData>,
}

impl Registry {
    #[inline]
    pub(crate) fn get_instantiator(&self, type_id: &TypeId) -> Option<BoxedCloneAsyncInstantiator<ResolveErrorKind, InstantiateErrorKind>> {
        self.instantiators.get(type_id).map(|data| data.instantiator.clone())
    }

    #[inline]
    pub(crate) fn get_instantiator_data(&self, type_id: &TypeId) -> Option<InstantiatorInnerData> {
        self.instantiators.get(type_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::RegistriesBuilder;
    use crate::{
        scope::DefaultScope::{self, *},
        Scopes,
    };

    use alloc::sync::Arc;
    use core::any::TypeId;

    #[test]
    fn test_build_empty() {
        let (registries, sync_registries) = RegistriesBuilder::<DefaultScope>::new().build();
        assert!(!registries.is_empty());
        assert!(!sync_registries.is_empty());
    }

    #[test]
    fn test_build_equal_provides() {
        let (registries, sync_registries) = RegistriesBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(()), App)
            .provide(|| Ok(()), App)
            .provide_async(async || Ok(((), ())), Runtime)
            .provide_async(async || Ok(((), ())), Runtime)
            .provide_async(async || Ok(((), ())), App)
            .provide_async(async || Ok(((), ())), App)
            .build();
        assert_eq!(registries.len(), DefaultScope::all().len());
        assert_eq!(sync_registries.len(), DefaultScope::all().len());

        for registry in registries {
            if registry.scope.priority == 1 {
                assert_eq!(registry.instantiators.len(), 1);
            } else {
                assert_eq!(registry.instantiators.len(), 0);
            }
        }
        for registry in sync_registries {
            if registry.scope.priority == 1 {
                assert_eq!(registry.instantiators.len(), 1);
            } else {
                assert_eq!(registry.instantiators.len(), 0);
            }
        }
    }

    #[test]
    fn test_build_several_scopes() {
        let (registries, sync_registries) = RegistriesBuilder::new()
            .provide(|| Ok(1i8), Runtime)
            .provide(|| Ok(1i16), Runtime)
            .provide(|| Ok(1i32), App)
            .provide(|| Ok(1i64), App)
            .provide_async(async || Ok(1u8), Runtime)
            .provide_async(async || Ok(1u16), Runtime)
            .provide_async(async || Ok(1u32), App)
            .provide_async(async || Ok(1u64), App)
            .build();
        assert_eq!(registries.len(), DefaultScope::all().len());
        assert_eq!(sync_registries.len(), DefaultScope::all().len());

        for registry in registries {
            if registry.scope.priority == 0 || registry.scope.priority == 1 {
                assert_eq!(registry.instantiators.len(), 2);
            } else {
                assert_eq!(registry.instantiators.len(), 0);
            }
        }
        for registry in sync_registries {
            if registry.scope.priority == 0 || registry.scope.priority == 1 {
                assert_eq!(registry.instantiators.len(), 2);
            } else {
                assert_eq!(registry.instantiators.len(), 0);
            }
        }
    }

    #[test]
    fn test_add_finalizer() {
        let (registries, sync_registries) = RegistriesBuilder::new()
            .provide(|| Ok(1i8), Runtime)
            .provide(|| Ok(1i16), Runtime)
            .provide(|| Ok(1i32), App)
            .provide(|| Ok(1i64), App)
            .provide_async(async || Ok(1u8), Runtime)
            .provide_async(async || Ok(1u16), Runtime)
            .provide_async(async || Ok(1u32), App)
            .provide_async(async || Ok(1u64), App)
            .add_finalizer(|_: Arc<i8>| {})
            .add_finalizer(|_: Arc<i32>| {})
            .add_async_finalizer(async |_: Arc<u8>| {})
            .add_async_finalizer(async |_: Arc<u32>| {})
            .build();

        let i8_type_id = TypeId::of::<i8>();
        let i16_type_id = TypeId::of::<i16>();
        let i32_type_id = TypeId::of::<i32>();
        let i64_type_id = TypeId::of::<i64>();
        let u8_type_id = TypeId::of::<u8>();
        let u16_type_id = TypeId::of::<u16>();
        let u32_type_id = TypeId::of::<u32>();
        let u64_type_id = TypeId::of::<u64>();

        for registry in registries {
            if let Some(data) = registry.instantiators.get(&u8_type_id) {
                assert!(data.finalizer.is_some());
                continue;
            }
            if let Some(data) = registry.instantiators.get(&u16_type_id) {
                assert!(data.finalizer.is_none());
                continue;
            }
            if let Some(data) = registry.instantiators.get(&u32_type_id) {
                assert!(data.finalizer.is_some());
                continue;
            }
            if let Some(data) = registry.instantiators.get(&u64_type_id) {
                assert!(data.finalizer.is_none());
            }
        }
        for registry in sync_registries {
            if let Some(data) = registry.instantiators.get(&i8_type_id) {
                assert!(data.finalizer.is_some());
                continue;
            }
            if let Some(data) = registry.instantiators.get(&i16_type_id) {
                assert!(data.finalizer.is_none());
                continue;
            }
            if let Some(data) = registry.instantiators.get(&i32_type_id) {
                assert!(data.finalizer.is_some());
                continue;
            }
            if let Some(data) = registry.instantiators.get(&i64_type_id) {
                assert!(data.finalizer.is_none());
            }
        }
    }
}
