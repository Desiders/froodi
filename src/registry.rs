use alloc::collections::BTreeMap;
use core::any::TypeId;

use crate::{
    dependency_resolver::ResolveErrorKind,
    instantiator::{BoxedCloneInstantiatorSync, InstantiateErrorKind},
};

#[derive(Default)]
pub(crate) struct Registry {
    instantiators:
        BTreeMap<TypeId, BoxedCloneInstantiatorSync<ResolveErrorKind, InstantiateErrorKind>>,
}

impl Registry {
    pub(crate) fn add_instantiator<Dep: 'static>(
        &mut self,
        value: BoxedCloneInstantiatorSync<ResolveErrorKind, InstantiateErrorKind>,
    ) -> Option<BoxedCloneInstantiatorSync<ResolveErrorKind, InstantiateErrorKind>> {
        self.instantiators.insert(TypeId::of::<Dep>(), value)
    }

    #[must_use]
    pub(crate) fn get_instantiator<Dep: 'static>(
        &self,
    ) -> Option<BoxedCloneInstantiatorSync<ResolveErrorKind, InstantiateErrorKind>> {
        self.instantiators.get(&TypeId::of::<Dep>()).cloned()
    }
}
