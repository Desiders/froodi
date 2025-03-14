use alloc::collections::BTreeMap;
use core::any::TypeId;

use super::instantiator::{BoxedCloneInstantiator, Config};
use crate::errors::{InstantiateErrorKind, ResolveErrorKind};

#[derive(Default)]
pub(crate) struct Registry {
    instantiators: BTreeMap<TypeId, (BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>, Config)>,
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
    pub(crate) fn get_instantiator<Dep: 'static>(
        &self,
    ) -> Option<(BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>, Config)> {
        self.instantiators.get(&TypeId::of::<Dep>()).cloned()
    }
}
