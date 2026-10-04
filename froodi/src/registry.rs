#[macro_use]
mod tuples;

pub(crate) mod boundary;
pub(crate) mod frontend;
pub(crate) mod linking;
mod macros;
pub(crate) mod registration;
pub(crate) mod static_scope;
pub(crate) mod topology;

#[cfg(test)]
mod tests;

pub use boundary::declare;
pub(crate) use frontend::{finish, IntoRegistry, RegistrationId};

use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    vec::Vec,
};

#[cfg(test)]
extern crate std;

#[cfg(test)]
std::thread_local! {
    pub(crate) static CYCLE_CHECKS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

use crate::{
    any::TypeInfo,
    dependency::{Dependency, EMPTY_DEPENDENCIES},
    errors::ValidationErrorKind,
    finalizer::BoxedCloneFinalizer,
    instantiator::{boxed_container_instantiator, RegistrationInstantiator},
    scope::{ScopeData, ScopeDataWithChildScopesData},
    Config, Container, DefaultScope, Scope, Scopes,
};

#[derive(Clone)]
pub(crate) struct RegistrationMetadata {
    pub(crate) dependencies: BTreeSet<Dependency>,
    pub(crate) config: Config,
    pub(crate) scope_data: ScopeData,
}

#[derive(Clone)]
pub struct InstantiatorData {
    pub(crate) instantiator: RegistrationInstantiator,
    pub(crate) finalizer: Option<BoxedCloneFinalizer>,
    pub(crate) metadata: RegistrationMetadata,
}

#[cfg(feature = "async")]
#[derive(Clone)]
pub struct AsyncInstantiatorData {
    pub(crate) instantiator: crate::async_impl::instantiator::RegistrationInstantiator,
    pub(crate) finalizer: Option<crate::async_impl::finalizer::BoxedCloneFinalizer>,
    pub(crate) metadata: RegistrationMetadata,
}

#[derive(Clone)]
pub(crate) enum Selected {
    Sync(InstantiatorData),
    #[cfg(feature = "async")]
    Async(AsyncInstantiatorData),
    Missing,
}

impl Selected {
    fn metadata(&self) -> Option<(&BTreeSet<Dependency>, ScopeData)> {
        match self {
            Self::Sync(data) => Some((&data.metadata.dependencies, data.metadata.scope_data)),
            #[cfg(feature = "async")]
            Self::Async(data) => Some((&data.metadata.dependencies, data.metadata.scope_data)),
            Self::Missing => None,
        }
    }

    pub(crate) fn link_keys(&mut self, keys: &[TypeInfo]) {
        match self {
            Selected::Sync(data) => link_sync_keys(data, keys),
            #[cfg(feature = "async")]
            Selected::Async(data) => {
                if let crate::async_impl::instantiator::RegistrationInstantiator::Linked(executor) = &mut data.instantiator {
                    executor.keys = executor.edges.iter().map(|id| keys[id.index()].clone()).collect::<Vec<_>>().into();
                    data.metadata.dependencies = executor.keys.iter().cloned().map(|type_info| Dependency { type_info }).collect();
                }
            }
            Selected::Missing => (),
        }
    }

    fn is_async(&self) -> bool {
        #[cfg(feature = "async")]
        if matches!(self, Self::Async(_)) {
            return true;
        }
        false
    }
}

#[derive(Clone)]
pub(crate) struct Entry {
    pub(crate) selected: Selected,
    // Erased composition can supply both execution kinds for one output type.
    // Async lookup prefers its async provider; sync lookup retains its own provider.
    #[cfg(feature = "async")]
    sync: Option<alloc::boxed::Box<InstantiatorData>>,
}

impl Entry {
    pub(crate) fn sync(&self) -> Option<&InstantiatorData> {
        match &self.selected {
            Selected::Sync(data) => Some(data),
            #[cfg(feature = "async")]
            Selected::Async(_) => self.sync.as_deref(),
            Selected::Missing => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn sync_mut(&mut self) -> Option<&mut InstantiatorData> {
        match &mut self.selected {
            Selected::Sync(data) => Some(data),
            #[cfg(feature = "async")]
            Selected::Async(_) => self.sync.as_deref_mut(),
            Selected::Missing => None,
        }
    }

    fn replace(&mut self, selected: Selected) {
        #[cfg(feature = "async")]
        match selected {
            Selected::Sync(data) if self.selected.is_async() => {
                self.sync = Some(alloc::boxed::Box::new(data));
                return;
            }
            Selected::Async(data) => {
                if let Selected::Sync(sync) = core::mem::replace(&mut self.selected, Selected::Async(data)) {
                    self.sync = Some(alloc::boxed::Box::new(sync));
                }
                return;
            }
            selected => self.selected = selected,
        }
        #[cfg(not(feature = "async"))]
        {
            self.selected = selected;
        }
    }

    pub(crate) fn from_selected(selected: Selected) -> Self {
        Self {
            selected,
            #[cfg(feature = "async")]
            sync: None,
        }
    }

    pub(crate) fn remap(&mut self, ids: &BTreeMap<TypeInfo, RegistrationId>) {
        match &mut self.selected {
            Selected::Sync(data) => remap_sync(data, ids),
            #[cfg(feature = "async")]
            Selected::Async(data) => {
                if let crate::async_impl::instantiator::RegistrationInstantiator::Linked(executor) = &mut data.instantiator {
                    executor.remap(ids);
                }
                if let Some(sync) = &mut self.sync {
                    remap_sync(sync, ids);
                }
            }
            Selected::Missing => (),
        }
    }

    pub(crate) fn linked_keys(&self) -> &[TypeInfo] {
        match &self.selected {
            Selected::Sync(data) => {
                if let RegistrationInstantiator::Linked(executor) = &data.instantiator {
                    return &executor.keys;
                }
            }
            #[cfg(feature = "async")]
            Selected::Async(data) => {
                if let crate::async_impl::instantiator::RegistrationInstantiator::Linked(executor) = &data.instantiator {
                    return &executor.keys;
                }
            }
            Selected::Missing => (),
        }
        &[]
    }

    pub(crate) fn fallback_keys(&self) -> &[TypeInfo] {
        #[cfg(feature = "async")]
        if let Some(data) = &self.sync {
            if let RegistrationInstantiator::Linked(executor) = &data.instantiator {
                return &executor.keys;
            }
        }
        &[]
    }

    pub(crate) fn is_linked(&self) -> bool {
        if let Some(data) = self.sync() {
            if matches!(data.instantiator, RegistrationInstantiator::Linked(_)) {
                return true;
            }
        }
        #[cfg(feature = "async")]
        if let Selected::Async(data) = &self.selected {
            return matches!(
                data.instantiator,
                crate::async_impl::instantiator::RegistrationInstantiator::Linked(_)
            );
        }
        false
    }
}

fn link_sync_keys(data: &mut InstantiatorData, keys: &[TypeInfo]) {
    if let RegistrationInstantiator::Linked(executor) = &mut data.instantiator {
        executor.keys = executor.edges.iter().map(|id| keys[id.index()].clone()).collect::<Vec<_>>().into();
        data.metadata.dependencies = executor.keys.iter().cloned().map(|type_info| Dependency { type_info }).collect();
    }
}

fn remap_sync(data: &mut InstantiatorData, ids: &BTreeMap<TypeInfo, RegistrationId>) {
    if let RegistrationInstantiator::Linked(executor) = &mut data.instantiator {
        executor.remap(ids);
    }
}

/// An erased registry containing sync, async or mixed registrations.
/// Use `.into_registry()` when returning or composing a runtime fragment.
///
/// Both container APIs accept this type. Sync containers resolve only sync
/// providers; async containers resolve either kind. Composition retains runtime
/// validation and remaps linked dependency edges to the selected registrations.
/// When erased fragments provide both kinds for one type, async requests prefer
/// the async provider and sync requests use the sync provider.
#[derive(Clone, Default)]
pub struct Registry {
    pub(crate) entries: BTreeMap<TypeInfo, Entry>,
    pub(crate) scopes_data: Vec<ScopeData>,
    pub(crate) indexed: Vec<(TypeInfo, Entry)>,
}

impl Registry {
    #[allow(clippy::similar_names)]
    pub(crate) fn new<T, S, const N: usize>(entries: BTreeMap<TypeInfo, InstantiatorData>) -> Self
    where
        S: Scope,
        T: Scopes<N, Scope = S>,
    {
        let (scope, child_scopes) = T::all();
        let scope_data = scope.into();
        let mut registry = Self::default();
        for (key, data) in entries {
            registry.insert(key, Selected::Sync(data));
        }
        registry.insert(
            TypeInfo::new::<Container>("Container"),
            Selected::Sync(InstantiatorData {
                instantiator: boxed_container_instantiator().into(),
                finalizer: None,
                metadata: RegistrationMetadata {
                    dependencies: EMPTY_DEPENDENCIES,
                    config: Config { cache_provides: false },
                    scope_data,
                },
            }),
        );
        #[cfg(feature = "async")]
        registry.insert(
            TypeInfo::new::<crate::async_impl::Container>("async_impl::Container"),
            Selected::Async(AsyncInstantiatorData {
                instantiator: crate::async_impl::instantiator::boxed_container_instantiator().into(),
                finalizer: None,
                metadata: RegistrationMetadata {
                    dependencies: EMPTY_DEPENDENCIES,
                    config: Config { cache_provides: false },
                    scope_data,
                },
            }),
        );
        registry.scopes_data.push(scope_data);
        registry.scopes_data.extend(child_scopes.into_iter().map(Into::into));
        registry
    }

    #[inline]
    #[must_use]
    pub fn new_with_default_entries() -> Self {
        Self::new::<DefaultScope, DefaultScope, 5>(BTreeMap::new())
    }

    pub(crate) fn insert(&mut self, key: TypeInfo, selected: Selected) {
        match self.entries.entry(key) {
            alloc::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(Entry::from_selected(selected));
            }
            alloc::collections::btree_map::Entry::Occupied(mut entry) => entry.get_mut().replace(selected),
        }
    }

    pub(crate) fn extend(&mut self, other: Self) {
        for (key, entry) in other.entries {
            #[cfg(feature = "async")]
            if let Some(sync) = entry.sync {
                self.insert(key.clone(), Selected::Sync(*sync));
            }
            self.insert(key, entry.selected);
        }
    }

    #[inline]
    pub(crate) fn get(&self, type_info: &TypeInfo) -> Option<&InstantiatorData> {
        self.entries.get(type_info).and_then(Entry::sync)
    }

    pub(crate) fn unavailable(&self, type_info: TypeInfo) -> crate::ResolveErrorKind {
        #[cfg(feature = "async")]
        if self.get_async(&type_info).is_some() {
            return crate::ResolveErrorKind::AsyncRequired { type_info };
        }
        crate::ResolveErrorKind::NoInstantiator { type_info }
    }

    #[cfg(feature = "async")]
    #[inline]
    pub(crate) fn get_async(&self, type_info: &TypeInfo) -> Option<&AsyncInstantiatorData> {
        match &self.entries.get(type_info)?.selected {
            Selected::Async(data) => Some(data),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn get_scope_with_child_scopes(&self) -> ScopeDataWithChildScopesData {
        ScopeDataWithChildScopesData::new_with_sort(self.scopes_data.clone())
    }

    pub fn validate(&self) -> Result<(), ValidationErrorKind> {
        self.detect_cyclic_dependencies()?;
        self.detect_unreachable_scopes()
    }

    pub(crate) fn detect_cyclic_dependencies(&self) -> Result<(), ValidationErrorKind> {
        #[cfg(test)]
        CYCLE_CHECKS.with(|checks| checks.set(checks.get() + 1));
        let mut visited = BTreeSet::new();
        let mut stack = Vec::new();
        for (key, entry) in &self.entries {
            for is_async in [false, true] {
                if self.metadata(key, is_async).is_some() && self.dfs_visit(key, is_async, &mut visited, &mut stack) {
                    let mut graph = stack.into_iter().map(|(key, _)| key).collect::<Vec<_>>();
                    return Err(ValidationErrorKind::CyclicDependency {
                        graph: (graph.remove(0), graph.into_boxed_slice()),
                    });
                }
                if !entry.selected.is_async() {
                    break;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn detect_unreachable_scopes(&self) -> Result<(), ValidationErrorKind> {
        for (key, entry) in &self.entries {
            for is_async in [false, true] {
                if let Some((dependencies, scope)) = self.metadata(key, is_async) {
                    for dependency in dependencies {
                        let target = self.entries.get(&dependency.type_info);
                        let target_metadata = if is_async {
                            target.and_then(|entry| entry.selected.metadata())
                        } else {
                            target
                                .and_then(Entry::sync)
                                .map(|data| (&data.metadata.dependencies, data.metadata.scope_data))
                        };
                        if let Some((_, dependency_scope)) = target_metadata {
                            if !scope.can_access(&dependency_scope) {
                                return Err(ValidationErrorKind::UnreachableDependency {
                                    dependent: key.clone(),
                                    dependent_scope: scope,
                                    dependency: dependency.type_info.clone(),
                                    dependency_scope,
                                });
                            }
                        } else if target.is_some() {
                            return Err(ValidationErrorKind::AsyncDependency {
                                dependent: key.clone(),
                                dependency: dependency.type_info.clone(),
                            });
                        }
                    }
                }
                if !entry.selected.is_async() {
                    break;
                }
            }
        }
        Ok(())
    }

    fn metadata(&self, key: &TypeInfo, is_async: bool) -> Option<(&BTreeSet<Dependency>, ScopeData)> {
        let entry = self.entries.get(key)?;
        if is_async {
            entry.selected.metadata()
        } else {
            entry.sync().map(|data| (&data.metadata.dependencies, data.metadata.scope_data))
        }
    }

    fn dfs_visit(
        &self,
        key: &TypeInfo,
        is_async: bool,
        visited: &mut BTreeSet<(TypeInfo, bool)>,
        stack: &mut Vec<(TypeInfo, bool)>,
    ) -> bool {
        let node = (key.clone(), is_async);
        if visited.contains(&node) {
            return false;
        }
        if stack.contains(&node) {
            return true;
        }
        stack.push(node.clone());
        if let Some((dependencies, _)) = self.metadata(key, is_async) {
            for dependency in dependencies {
                if let Some(target) = self.entries.get(&dependency.type_info) {
                    let target_async = is_async && target.selected.is_async();
                    if self.metadata(&dependency.type_info, target_async).is_some()
                        && self.dfs_visit(&dependency.type_info, target_async, visited, stack)
                    {
                        return true;
                    }
                }
            }
        }
        stack.pop();
        visited.insert(node);
        false
    }
}

pub(crate) enum Selection<'a, Data> {
    ByType,
    Indexed(Option<&'a Data>),
}

impl<'a, Data> Selection<'a, Data> {
    pub(crate) fn or_lookup(self, lookup: impl FnOnce() -> Option<&'a Data>) -> Option<&'a Data> {
        match self {
            Self::ByType => lookup(),
            Self::Indexed(data) => data,
        }
    }
}
