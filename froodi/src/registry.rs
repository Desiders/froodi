use alloc::{collections::BTreeMap, vec::Vec};
use core::any::TypeId;

use super::{
    errors::{InstantiateErrorKind, ResolveErrorKind},
    instantiator::BoxedCloneInstantiator,
    Config,
};
use crate::{
    dependency_resolver::DependencyResolver,
    finalizer::{boxed_finalizer_factory, BoxedCloneFinalizer, Finalizer},
    instantiator::{boxed_container_instantiator, boxed_instantiator, Instantiator},
    scope::{Scope, ScopeData},
    utils::thread_safety::{SendSafety, SyncSafety},
    Container, DefaultScope, Scopes as ScopesTrait,
};

#[derive(Clone)]
pub(crate) struct InstantiatorData {
    pub(crate) instantiator: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,
    pub(crate) finalizer: Option<BoxedCloneFinalizer>,
    pub(crate) config: Config,
    pub(crate) scope_data: ScopeData,
}

pub struct RegistryBuilder<S> {
    instantiators: BTreeMap<TypeId, InstantiatorData>,
    finalizers: BTreeMap<TypeId, BoxedCloneFinalizer>,
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
        self.add_instantiator::<Inst::Provides>(boxed_instantiator(instantiator), scope);
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
    pub fn add_finalizer<Dep>(mut self, finalizer: impl Finalizer<Dep> + SendSafety + SyncSafety) -> Self
    where
        Dep: SendSafety + SyncSafety + 'static,
    {
        self.finalizers.insert(TypeId::of::<Dep>(), boxed_finalizer_factory(finalizer));
        self
    }
}

impl<S> RegistryBuilder<S>
where
    S: Scope,
{
    #[inline]
    pub(crate) fn add_instantiator<Dep: 'static>(
        &mut self,
        instantiator: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,
        scope: S,
    ) -> Option<InstantiatorData> {
        self.add_instantiator_with_config::<Dep>(instantiator, Config::default(), scope)
    }

    #[inline]
    pub(crate) fn add_instantiator_with_config<Dep: 'static>(
        &mut self,
        instantiator: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,
        config: Config,
        scope: S,
    ) -> Option<InstantiatorData> {
        self.instantiators.insert(
            TypeId::of::<Dep>(),
            InstantiatorData {
                instantiator,
                finalizer: None,
                config,
                scope_data: scope.into(),
            },
        )
    }
}

pub(crate) struct RegistryWithScopes {
    pub(crate) registry: Registry,
    pub(crate) scope_data: ScopeData,
    pub(crate) child_scopes_data: Vec<ScopeData>,
}

impl<S> RegistryBuilder<S>
where
    S: Scope,
{
    pub(crate) fn build(mut self) -> RegistryWithScopes {
        let mut scope_iter = self.scopes.into_iter();
        let scope_data = scope_iter.next().expect("registries len (is 0) should be > 0").into();
        let child_scopes_data = scope_iter.map(Into::into).collect();

        RegistryWithScopes {
            registry: Registry {
                instantiators: self
                    .instantiators
                    .into_iter()
                    .map(
                        |(
                            type_id,
                            InstantiatorData {
                                instantiator,
                                finalizer,
                                config,
                                scope_data,
                            },
                        )| {
                            (
                                type_id,
                                InstantiatorData {
                                    instantiator,
                                    finalizer: finalizer.or(self.finalizers.remove(&type_id)),
                                    config,
                                    scope_data,
                                },
                            )
                        },
                    )
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
            },
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
    use super::{Registry, RegistryBuilder, RegistryWithScopes};
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
            child_scopes_data,
            ..
        } = RegistryBuilder::<DefaultScope>::new().build();
        assert_eq!(instantiators.len(), 1);
        assert!(!child_scopes_data.is_empty());
    }

    #[test]
    fn test_build_equal_provides() {
        let RegistryWithScopes {
            registry,
            child_scopes_data,
            ..
        } = RegistryBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(()), App)
            .provide(|| Ok(()), App)
            .build();
        assert_eq!(child_scopes_data.len() + 1, DefaultScope::all().len());
        assert_eq!(registry.instantiators.len(), 2);
    }

    #[test]
    fn test_build_several_scopes() {
        let RegistryWithScopes {
            registry,
            child_scopes_data,
            ..
        } = RegistryBuilder::new()
            .provide(|| Ok(1i8), Runtime)
            .provide(|| Ok(1i16), Runtime)
            .provide(|| Ok(1i32), App)
            .provide(|| Ok(1i64), App)
            .build();
        assert_eq!(child_scopes_data.len() + 1, DefaultScope::all().len());
        assert_eq!(registry.instantiators.len(), 5);
    }

    #[test]
    fn test_add_finalizer() {
        let RegistryWithScopes {
            registry: Registry { instantiators },
            ..
        } = RegistryBuilder::new()
            .provide(|| Ok(1i8), Runtime)
            .provide(|| Ok(1i16), Runtime)
            .provide(|| Ok(1i32), App)
            .provide(|| Ok(1i64), App)
            .add_finalizer(|_: RcThreadSafety<i8>| {})
            .add_finalizer(|_: RcThreadSafety<i32>| {})
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
    }
}
