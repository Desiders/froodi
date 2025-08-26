use alloc::{collections::BTreeMap, vec, vec::Vec};
use core::any::TypeId;

use super::{
    errors::{InstantiateErrorKind, ResolveErrorKind},
    instantiator::{BoxedCloneInstantiator, Config},
};
use crate::{
    dependency_resolver::DependencyResolver,
    finalizer::{boxed_finalizer_factory, BoxedCloneFinalizer, Finalizer},
    instantiator::{boxed_container_instantiator, boxed_instantiator, Instantiator},
    scope::{Scope, ScopeInnerData},
    Container, DefaultScope, Scopes as ScopesTrait,
};

pub(crate) struct InstantiatorData<S> {
    instantiator: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,
    config: Config,
    scope: S,
}

pub struct RegistryBuilder<Scope> {
    instantiators: BTreeMap<TypeId, InstantiatorData<Scope>>,
    finalizers: BTreeMap<TypeId, BoxedCloneFinalizer>,
    scopes: Vec<Scope>,
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
            scopes: Vec::from(DefaultScope::all()),
        }
    }
}

impl<Scope> RegistryBuilder<Scope> {
    #[inline]
    #[must_use]
    pub fn new_with_scopes<Scopes, const N: usize>() -> Self
    where
        Scopes: ScopesTrait<N, Scope = Scope>,
    {
        Self {
            instantiators: BTreeMap::new(),
            finalizers: BTreeMap::new(),
            scopes: Vec::from(Scopes::all()),
        }
    }
}

impl<S> RegistryBuilder<S> {
    #[inline]
    #[must_use]
    pub fn provide<Inst, Deps>(mut self, instantiator: Inst, scope: S) -> Self
    where
        Inst: Instantiator<Deps, Error = InstantiateErrorKind> + Send + Sync,
        Deps: DependencyResolver<Error = ResolveErrorKind>,
    {
        self.add_instantiator::<Inst::Provides>(boxed_instantiator(instantiator), scope);
        self
    }

    #[inline]
    #[must_use]
    pub fn provide_with_config<Inst, Deps>(mut self, instantiator: Inst, config: Config, scope: S) -> Self
    where
        Inst: Instantiator<Deps, Error = InstantiateErrorKind> + Send + Sync,
        Deps: DependencyResolver<Error = ResolveErrorKind>,
    {
        self.add_instantiator_with_config::<Inst::Provides>(boxed_instantiator(instantiator), config, scope);
        self
    }

    /// Adds a finalizer for the given a non transient dependency type.
    /// The finalizer will be called when the container is being closed in LIFO order of their usage (not the order of registration).
    ///
    /// # Warning
    /// - The finalizer can only be used for non-transient dependencies, because the transient doesn't have a lifetime and isn't cached.
    ///
    /// - [`Drop`] trait isn't a equivalent of a finalizer, because:
    ///     1. The finalizer is called in LIFO order of their usage, while [`Drop`] is called in the order of registration.
    ///     2. The finalized used for life cycle management, while [`Drop`] is used for resource management.
    #[inline]
    #[must_use]
    pub fn add_finalizer<Dep>(mut self, finalizer: impl Finalizer<Dep> + Send + Sync) -> Self
    where
        Dep: Send + Sync + 'static,
    {
        self.finalizers.insert(TypeId::of::<Dep>(), boxed_finalizer_factory(finalizer));
        self
    }
}

impl<S> RegistryBuilder<S> {
    #[inline]
    pub(crate) fn add_instantiator<Dep: 'static>(
        &mut self,
        instantiator: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,
        scope: S,
    ) -> Option<InstantiatorData<S>> {
        self.add_instantiator_with_config::<Dep>(instantiator, Config::default(), scope)
    }

    #[inline]
    pub(crate) fn add_instantiator_with_config<Dep: 'static>(
        &mut self,
        instantiator: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,
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

impl<S> RegistryBuilder<S>
where
    S: Scope,
{
    pub(crate) fn build(mut self) -> Vec<ScopedRegistry> {
        use alloc::collections::btree_map::Entry::{Occupied, Vacant};

        let mut scopes_instantiators: BTreeMap<S, Vec<(TypeId, InstantiatorInnerData)>> =
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
            let finalizer = self.finalizers.remove(&type_id);

            match scopes_instantiators.entry(scope) {
                Vacant(entry) => {
                    entry.insert(vec![(
                        type_id,
                        InstantiatorInnerData {
                            instantiator,
                            finalizer,
                            config,
                        },
                    )]);
                }
                Occupied(entry) => {
                    entry.into_mut().push((
                        type_id,
                        InstantiatorInnerData {
                            instantiator,
                            finalizer,
                            config,
                        },
                    ));
                }
            }
        }

        let container_type_id = TypeId::of::<Container>();
        let container_instantiator_data = InstantiatorInnerData {
            instantiator: boxed_container_instantiator(),
            finalizer: None,
            config: Config { cache_provides: true },
        };

        let mut registries = Vec::with_capacity(scopes_instantiators.len());
        for (scope, instantiators) in scopes_instantiators {
            let mut instantiators = BTreeMap::from_iter(instantiators);
            instantiators.insert(container_type_id, container_instantiator_data.clone());

            registries.push(ScopedRegistry {
                scope: ScopeInnerData {
                    priority: scope.priority(),
                    is_skipped_by_default: scope.is_skipped_by_default(),
                },
                instantiators,
            });
        }

        registries
    }
}

#[derive(Clone)]
pub(crate) struct InstantiatorInnerData {
    pub(crate) instantiator: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,
    pub(crate) finalizer: Option<BoxedCloneFinalizer>,
    pub(crate) config: Config,
}

pub(crate) struct ScopedRegistry {
    pub(crate) scope: ScopeInnerData,
    pub(crate) instantiators: BTreeMap<TypeId, InstantiatorInnerData>,
}

impl ScopedRegistry {
    #[inline]
    pub(crate) fn get_instantiator(&self, type_id: &TypeId) -> Option<BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>> {
        self.instantiators.get(type_id).map(|data| data.instantiator.clone())
    }

    #[inline]
    pub(crate) fn get_instantiator_data(&self, type_id: &TypeId) -> Option<InstantiatorInnerData> {
        self.instantiators.get(type_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::RegistryBuilder;
    use crate::{
        scope::DefaultScope::{self, *},
        Scopes,
    };

    use alloc::sync::Arc;
    use core::any::TypeId;

    #[test]
    fn test_build_empty() {
        let registries = RegistryBuilder::<DefaultScope>::new().build();
        assert!(!registries.is_empty());
    }

    #[test]
    fn test_build_equal_provides() {
        let registries = RegistryBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(()), App)
            .provide(|| Ok(()), App)
            .build();
        assert_eq!(registries.len(), DefaultScope::all().len());

        for registry in registries {
            if registry.scope.priority == 1 {
                assert_eq!(registry.instantiators.len(), 1);
            } else {
                assert_eq!(registry.instantiators.len(), 0);
            }
        }
    }

    #[test]
    fn test_build_several_scopes() {
        let registries = RegistryBuilder::new()
            .provide(|| Ok(1i8), Runtime)
            .provide(|| Ok(1i16), Runtime)
            .provide(|| Ok(1i32), App)
            .provide(|| Ok(1i64), App)
            .build();
        assert_eq!(registries.len(), DefaultScope::all().len());

        for registry in registries {
            if registry.scope.priority == 0 || registry.scope.priority == 1 {
                assert_eq!(registry.instantiators.len(), 2);
            } else {
                assert_eq!(registry.instantiators.len(), 0);
            }
        }
    }

    #[test]
    fn test_add_finalizer() {
        let registries = RegistryBuilder::new()
            .provide(|| Ok(1i8), Runtime)
            .provide(|| Ok(1i16), Runtime)
            .provide(|| Ok(1i32), App)
            .provide(|| Ok(1i64), App)
            .add_finalizer(|_: Arc<i8>| {})
            .add_finalizer(|_: Arc<i32>| {})
            .build();

        let i8_type_id = TypeId::of::<i8>();
        let i16_type_id = TypeId::of::<i16>();
        let i32_type_id = TypeId::of::<i32>();
        let i64_type_id = TypeId::of::<i64>();

        for registry in registries {
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
