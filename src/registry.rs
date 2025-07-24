use alloc::{collections::BTreeMap, vec, vec::Vec};
use core::any::TypeId;

use super::{
    errors::{InstantiateErrorKind, ResolveErrorKind},
    instantiator::{BoxedCloneInstantiator, Config},
};
use crate::{
    dependency_resolver::DependencyResolver,
    finalizer::{boxed_finalizer_factory, BoxedCloneFinalizer, Finalizer},
    instantiator::{boxed_instantiator_factory, Instantiator},
    scope::Scope,
};

pub(crate) struct InstantiatorData<S> {
    instantiator: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,
    config: Config,
    scope: S,
}

pub struct RegistriesBuilder<S> {
    instantiators: BTreeMap<TypeId, InstantiatorData<S>>,
    finalizers: BTreeMap<TypeId, BoxedCloneFinalizer>,
}

impl<S> Default for RegistriesBuilder<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> RegistriesBuilder<S> {
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            instantiators: BTreeMap::new(),
            finalizers: BTreeMap::new(),
        }
    }

    #[inline]
    #[allow(private_bounds)]
    #[must_use]
    pub fn provide<Inst, Deps>(mut self, instantiator: Inst, scope: S) -> Self
    where
        Inst: Instantiator<Deps, Error = InstantiateErrorKind> + Send + Sync,
        Deps: DependencyResolver<Error = ResolveErrorKind>,
    {
        self.add_instantiator::<Inst::Provides>(boxed_instantiator_factory(instantiator), scope);
        self
    }

    #[inline]
    #[allow(private_bounds)]
    #[must_use]
    pub fn provide_with_config<Inst, Deps>(mut self, instantiator: Inst, config: Config, scope: S) -> Self
    where
        Inst: Instantiator<Deps, Error = InstantiateErrorKind> + Send + Sync,
        Deps: DependencyResolver<Error = ResolveErrorKind>,
    {
        self.add_instantiator_with_config::<Inst::Provides>(boxed_instantiator_factory(instantiator), config, scope);
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
    pub fn add_finalizer<Dep, Fin>(mut self, finalizer: Fin) -> Self
    where
        Dep: Send + Sync + 'static,
        Fin: Finalizer<Dep> + Send + Sync,
    {
        self.finalizers.insert(TypeId::of::<Dep>(), boxed_finalizer_factory(finalizer));
        self
    }
}

impl<S> RegistriesBuilder<S> {
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

impl<S> RegistriesBuilder<S>
where
    S: Scope,
{
    pub(crate) fn build(mut self) -> Vec<Registry> {
        use alloc::collections::btree_map::Entry::{Occupied, Vacant};

        let mut scopes_instantiators: BTreeMap<S, Vec<(TypeId, InstantiatorInnerData)>> = BTreeMap::new();
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

        registries
    }
}

#[cfg_attr(feature = "debug", derive(Debug))]
pub(crate) struct ScopeInnerData {
    pub(crate) priority: u8,
    pub(crate) is_skipped_by_default: bool,
}

#[derive(Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub(crate) struct InstantiatorInnerData {
    pub(crate) instantiator: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,
    pub(crate) finalizer: Option<BoxedCloneFinalizer>,
    pub(crate) config: Config,
}

#[cfg_attr(feature = "debug", derive(Debug))]
pub(crate) struct Registry {
    pub(crate) scope: ScopeInnerData,
    instantiators: BTreeMap<TypeId, InstantiatorInnerData>,
}

impl Registry {
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
    use super::RegistriesBuilder;
    use crate::scope::DefaultScope::{self, *};

    use alloc::sync::Arc;
    use core::any::TypeId;

    #[test]
    fn test_build_empty() {
        let registries = RegistriesBuilder::<DefaultScope>::new().build();
        assert!(registries.is_empty());
    }

    #[test]
    fn test_build_equal_provides() {
        let registries = RegistriesBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(()), App)
            .provide(|| Ok(()), App)
            .build();
        assert_eq!(registries.len(), 1);

        for registry in registries {
            assert_eq!(registry.instantiators.len(), 1);
        }
    }

    #[test]
    fn test_build_several_scopes() {
        let registries = RegistriesBuilder::new()
            .provide(|| Ok(1i8), Runtime)
            .provide(|| Ok(1i16), Runtime)
            .provide(|| Ok(1i32), App)
            .provide(|| Ok(1i64), App)
            .build();
        assert_eq!(registries.len(), 2);

        for registry in registries {
            assert_eq!(registry.instantiators.len(), 2);
        }
    }

    #[test]
    fn test_add_finalizer() {
        let registries = RegistriesBuilder::new()
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
