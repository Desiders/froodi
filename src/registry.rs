use alloc::{collections::BTreeMap, vec, vec::Vec};
use core::any::TypeId;

use super::{
    errors::{InstantiateErrorKind, ResolveErrorKind},
    instantiator::{BoxedCloneInstantiator, Config},
};
use crate::{
    dependency_resolver::DependencyResolver,
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
}

impl<S> RegistriesBuilder<S> {
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            instantiators: BTreeMap::new(),
        }
    }

    #[inline]
    #[allow(private_bounds)]
    #[must_use]
    pub fn provide<Inst, Deps>(mut self, instantiator: Inst, scope: S) -> Self
    where
        Inst: Instantiator<Deps, Error = InstantiateErrorKind>,
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
        Inst: Instantiator<Deps, Error = InstantiateErrorKind>,
        Deps: DependencyResolver<Error = ResolveErrorKind>,
    {
        self.add_instantiator_with_config::<Inst::Provides>(boxed_instantiator_factory(instantiator), config, scope);
        self
    }
}

impl<S> RegistriesBuilder<S> {
    pub(crate) fn add_instantiator<Dep: 'static>(
        &mut self,
        instantiator: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,
        scope: S,
    ) -> Option<InstantiatorData<S>> {
        self.instantiators.insert(
            TypeId::of::<Dep>(),
            InstantiatorData {
                instantiator,
                scope,
                config: Config::default(),
            },
        )
    }

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
    pub(crate) fn build(self) -> Vec<Registry> {
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
            match scopes_instantiators.entry(scope) {
                Vacant(entry) => {
                    entry.insert(vec![(type_id, InstantiatorInnerData { instantiator, config })]);
                }
                Occupied(entry) => {
                    entry.into_mut().push((type_id, InstantiatorInnerData { instantiator, config }));
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
    pub(crate) config: Config,
}

#[cfg_attr(feature = "debug", derive(Debug))]
pub(crate) struct Registry {
    pub(crate) scope: ScopeInnerData,
    instantiators: BTreeMap<TypeId, InstantiatorInnerData>,
}

impl Registry {
    #[must_use]
    pub(crate) fn get_instantiator<Dep: 'static>(&self) -> Option<BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>> {
        self.instantiators.get(&TypeId::of::<Dep>()).map(|data| data.instantiator.clone())
    }

    #[must_use]
    pub(crate) fn get_instantiator_data<Dep: 'static>(&self) -> Option<InstantiatorInnerData> {
        self.instantiators.get(&TypeId::of::<Dep>()).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::RegistriesBuilder;
    use crate::scope::DefaultScope::{self, *};

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
}
