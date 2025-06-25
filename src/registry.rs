use alloc::collections::BTreeMap;
use core::any::TypeId;

use super::{
    errors::{InstantiateErrorKind, ResolveErrorKind},
    instantiator::{BoxedCloneInstantiator, Config},
};
use crate::{
    dependency_resolver::DependencyResolver,
    instantiator::{boxed_instantiator_factory, Instantiator},
};

#[derive(Default, Clone)]
pub struct Registry {
    instantiators: BTreeMap<TypeId, (BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>, Config)>,
}

impl Registry {
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
    pub fn provide<Inst, Deps>(mut self, instantiator: Inst) -> Self
    where
        Inst: Instantiator<Deps, Error = InstantiateErrorKind>,
        Deps: DependencyResolver<Error = ResolveErrorKind>,
    {
        self.add_instantiator::<Inst::Provides>(boxed_instantiator_factory(instantiator));
        self
    }

    #[inline]
    #[allow(private_bounds)]
    #[must_use]
    pub fn provide_with_config<Inst, Deps>(mut self, instantiator: Inst, config: Config) -> Self
    where
        Inst: Instantiator<Deps, Error = InstantiateErrorKind>,
        Deps: DependencyResolver<Error = ResolveErrorKind>,
    {
        self.add_instantiator_with_config::<Inst::Provides>(boxed_instantiator_factory(instantiator), config);
        self
    }
}

impl Registry {
    pub(crate) fn add_instantiator<Dep: 'static>(
        &mut self,
        value: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,
    ) -> Option<(BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>, Config)> {
        self.instantiators.insert(TypeId::of::<Dep>(), (value, Config::default()))
    }

    pub(crate) fn add_instantiator_with_config<Dep: 'static>(
        &mut self,
        value: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,
        config: Config,
    ) -> Option<(BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>, Config)> {
        self.instantiators.insert(TypeId::of::<Dep>(), (value, config))
    }

    #[must_use]
    pub(crate) fn get_instantiator<Dep: 'static>(&self) -> Option<BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>> {
        self.instantiators.get(&TypeId::of::<Dep>()).map(|(value, _)| value.clone())
    }

    #[must_use]
    pub(crate) fn get_instantiator_with_config<Dep: 'static>(
        &self,
    ) -> Option<(BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>, Config)> {
        self.instantiators.get(&TypeId::of::<Dep>()).cloned()
    }
}
