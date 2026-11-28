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
    collections::{btree_map::Entry as BTreeMapEntry, BTreeMap, BTreeSet},
    vec::Vec,
};

#[cfg(feature = "async")]
use alloc::boxed::Box;
#[cfg(feature = "async")]
use core::mem::replace;
use core::{any::TypeId, slice::Iter};

#[cfg(feature = "async")]
use crate::async_impl::{
    finalizer::BoxedCloneFinalizer as AsyncBoxedCloneFinalizer,
    instantiator::{
        boxed_container_instantiator as boxed_async_container_instantiator, RegistrationInstantiator as AsyncRegistrationInstantiator,
    },
    Container as AsyncContainer,
};

use crate::{
    any::TypeInfo,
    dependency::{Dependency, EMPTY_DEPENDENCIES},
    errors::{ResolveErrorKind, ValidationErrorKind},
    finalizer::BoxedCloneFinalizer,
    instantiator::{boxed_container_instantiator, RegistrationInstantiator},
    scope::{ScopeData, ScopeDataWithChildScopesData},
    utils::thread_safety::RcThreadSafety,
    Config, Container, DefaultScope, Scope, Scopes,
};

#[derive(Clone)]
pub(crate) struct RegistrationMetadata {
    #[cfg(all(
        feature = "std",
        feature = "thread_safe",
        not(feature = "lock-parking-lot"),
        not(feature = "lock-spin")
    ))]
    pub(crate) lock_id: RegistrationId,
    dependencies: Option<RcThreadSafety<[Dependency]>>,
    pub(crate) config: Config,
    pub(crate) scope_data: ScopeData,
}

impl RegistrationMetadata {
    #[inline]
    pub(crate) fn new(dependencies: BTreeSet<Dependency>, config: Config, scope_data: ScopeData) -> Self {
        Self {
            #[cfg(all(
                feature = "std",
                feature = "thread_safe",
                not(feature = "lock-parking-lot"),
                not(feature = "lock-spin")
            ))]
            lock_id: RegistrationId::UNINDEXED_LOCK,
            dependencies: if dependencies.is_empty() {
                None
            } else {
                Some(dependencies.into_iter().collect())
            },
            config,
            scope_data,
        }
    }

    fn dependencies(&self) -> &[Dependency] {
        self.dependencies.as_deref().unwrap_or(&[])
    }

    fn set_linked_dependencies(&mut self, keys: &[TypeInfo]) {
        if keys.is_empty() {
            self.dependencies = None;
            return;
        }

        // Preserve the last name for duplicate TypeIds, as BTreeSet collection did.
        let mut dependencies: Vec<_> = keys.iter().rev().cloned().map(|type_info| Dependency { type_info }).collect();
        dependencies.sort();
        dependencies.dedup();
        self.dependencies = Some(dependencies.into());
    }
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
    pub(crate) instantiator: AsyncRegistrationInstantiator,
    pub(crate) finalizer: Option<AsyncBoxedCloneFinalizer>,
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
    fn metadata(&self) -> Option<(&[Dependency], ScopeData)> {
        match self {
            Self::Sync(data) => Some((data.metadata.dependencies(), data.metadata.scope_data)),
            #[cfg(feature = "async")]
            Self::Async(data) => Some((data.metadata.dependencies(), data.metadata.scope_data)),
            Self::Missing => None,
        }
    }

    pub(crate) fn link_keys(&mut self, keys: &[TypeInfo]) {
        match self {
            Selected::Sync(data) => link_sync_keys(data, keys),
            #[cfg(feature = "async")]
            Selected::Async(data) => {
                if let AsyncRegistrationInstantiator::Linked(executor) = &mut data.instantiator {
                    executor.keys = executor.edges.iter().map(|id| keys[id.index()].clone()).collect::<Vec<_>>().into();
                    data.metadata.set_linked_dependencies(&executor.keys);
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
    sync: Option<Box<InstantiatorData>>,
}

impl Entry {
    fn metadata(&self, is_async: bool) -> Option<(&[Dependency], ScopeData)> {
        if is_async {
            self.selected.metadata()
        } else {
            self.sync().map(|data| (data.metadata.dependencies(), data.metadata.scope_data))
        }
    }

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
                self.sync = Some(Box::new(data));
                return;
            }
            Selected::Async(data) => {
                if let Selected::Sync(sync) = replace(&mut self.selected, Selected::Async(data)) {
                    self.sync = Some(Box::new(sync));
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
                if let AsyncRegistrationInstantiator::Linked(executor) = &mut data.instantiator {
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
                if let AsyncRegistrationInstantiator::Linked(executor) = &data.instantiator {
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
            return matches!(data.instantiator, AsyncRegistrationInstantiator::Linked(_));
        }
        false
    }
}

fn link_sync_keys(data: &mut InstantiatorData, keys: &[TypeInfo]) {
    if let RegistrationInstantiator::Linked(executor) = &mut data.instantiator {
        executor.keys = executor.edges.iter().map(|id| keys[id.index()].clone()).collect::<Vec<_>>().into();
        data.metadata.set_linked_dependencies(&executor.keys);
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
    #[cfg(all(
        feature = "std",
        feature = "thread_safe",
        not(feature = "lock-parking-lot"),
        not(feature = "lock-spin")
    ))]
    lock_counts: LockCounts,
}

#[cfg(all(
    feature = "std",
    feature = "thread_safe",
    not(feature = "lock-parking-lot"),
    not(feature = "lock-spin")
))]
#[derive(Clone, Default)]
struct LockCounts {
    sync: u32,
    asynchronous: u32,
}

struct CycleFrame<'a> {
    key: TypeInfo,
    is_async: bool,
    dependencies: Iter<'a, Dependency>,
}

impl Registry {
    #[cfg(all(
        feature = "std",
        feature = "thread_safe",
        not(feature = "lock-parking-lot"),
        not(feature = "lock-spin")
    ))]
    pub(crate) fn index_locks(&mut self) {
        fn next_id(count: &mut u32) -> RegistrationId {
            let id = RegistrationId(*count);
            *count = count.checked_add(1).expect("too many registrations");
            id
        }

        let mut counts = LockCounts::default();
        for entry in self.entries.values_mut() {
            match &mut entry.selected {
                Selected::Sync(data) => data.metadata.lock_id = next_id(&mut counts.sync),
                #[cfg(feature = "async")]
                Selected::Async(data) => data.metadata.lock_id = next_id(&mut counts.asynchronous),
                Selected::Missing => (),
            }
            #[cfg(feature = "async")]
            if let Some(data) = &mut entry.sync {
                data.metadata.lock_id = next_id(&mut counts.sync);
            }
        }
        self.lock_counts = counts;
    }

    #[cfg(all(
        feature = "std",
        feature = "thread_safe",
        not(feature = "lock-parking-lot"),
        not(feature = "lock-spin")
    ))]
    pub(crate) fn indexed_lock_keys(&self, is_async: bool) -> Option<(usize, impl Iterator<Item = TypeId> + '_)> {
        if self.indexed.is_empty() {
            return None;
        }

        let len = if is_async {
            self.lock_counts.asynchronous
        } else {
            self.lock_counts.sync
        };
        let keys = self.entries.iter().filter_map(move |(key, entry)| {
            let present = if is_async {
                entry.selected.is_async()
            } else {
                entry.sync().is_some()
            };
            present.then_some(key.id)
        });
        Some((len as usize, keys))
    }

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
                metadata: RegistrationMetadata::new(EMPTY_DEPENDENCIES, Config { cache_provides: false }, scope_data),
            }),
        );
        #[cfg(feature = "async")]
        registry.insert(
            TypeInfo::new::<AsyncContainer>("async_impl::Container"),
            Selected::Async(AsyncInstantiatorData {
                instantiator: boxed_async_container_instantiator().into(),
                finalizer: None,
                metadata: RegistrationMetadata::new(EMPTY_DEPENDENCIES, Config { cache_provides: false }, scope_data),
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
            BTreeMapEntry::Vacant(entry) => {
                entry.insert(Entry::from_selected(selected));
            }
            BTreeMapEntry::Occupied(mut entry) => entry.get_mut().replace(selected),
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

    #[inline]
    pub(crate) fn get_indexed<Dep: 'static>(&self, id: RegistrationId) -> Result<&Entry, ResolveErrorKind> {
        let (actual, entry) = &self.indexed[id.index()];
        if actual.id != TypeId::of::<Dep>() {
            return Err(ResolveErrorKind::IncorrectType {
                expected: TypeInfo::of::<Dep>(),
                actual: actual.clone(),
            });
        }
        Ok(entry)
    }

    pub(crate) fn unavailable(&self, type_info: TypeInfo) -> ResolveErrorKind {
        #[cfg(feature = "async")]
        if self.get_async(&type_info).is_some() {
            return ResolveErrorKind::AsyncRequired { type_info };
        }
        ResolveErrorKind::NoInstantiator { type_info }
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
    pub(crate) fn get_scope_with_child_scopes(&self) -> ScopeDataWithChildScopesData<'_> {
        ScopeDataWithChildScopesData::from_sorted(&self.scopes_data)
    }

    pub fn validate(&self) -> Result<(), ValidationErrorKind> {
        self.detect_cyclic_dependencies()?;
        self.detect_unreachable_scopes()
    }

    pub(crate) fn detect_cyclic_dependencies(&self) -> Result<(), ValidationErrorKind> {
        #[cfg(test)]
        tests::CYCLE_CHECKS.with(|checks| checks.set(checks.get() + 1));
        // true: active path; false: fully visited.
        let mut visited = BTreeMap::new();
        let mut stack = Vec::new();
        for (key, entry) in &self.entries {
            for is_async in [false, true] {
                if let Some((dependencies, _)) = entry.metadata(is_async) {
                    if self.dfs_visit(key, is_async, dependencies, &mut visited, &mut stack) {
                        let mut graph = stack.into_iter().map(|frame| frame.key).collect::<Vec<_>>();
                        return Err(ValidationErrorKind::CyclicDependency {
                            graph: (graph.remove(0), graph.into_boxed_slice()),
                        });
                    }
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
                if let Some((dependencies, scope)) = entry.metadata(is_async) {
                    for dependency in dependencies {
                        let target = self.entries.get(&dependency.type_info);
                        let target_metadata = target.and_then(|entry| entry.metadata(is_async));
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

    fn dfs_visit<'a>(
        &'a self,
        key: &TypeInfo,
        is_async: bool,
        dependencies: &'a [Dependency],
        visited: &mut BTreeMap<(TypeInfo, bool), bool>,
        stack: &mut Vec<CycleFrame<'a>>,
    ) -> bool {
        match visited.entry((key.clone(), is_async)) {
            BTreeMapEntry::Occupied(state) => return *state.get(),
            BTreeMapEntry::Vacant(state) => {
                state.insert(true);
            }
        }
        stack.push(CycleFrame {
            key: key.clone(),
            is_async,
            dependencies: dependencies.iter(),
        });
        // Preserve recursive dependency order.
        while let Some(frame) = stack.last_mut() {
            let Some(dependency) = frame.dependencies.next() else {
                let frame = stack.pop().expect("completed DFS frame exists");
                *visited.get_mut(&(frame.key, frame.is_async)).expect("DFS node remains recorded") = false;
                continue;
            };
            let type_info = &dependency.type_info;
            let Some(target) = self.entries.get(type_info) else {
                continue;
            };
            let target_async = frame.is_async && target.selected.is_async();
            let Some((dependencies, _)) = target.metadata(target_async) else {
                continue;
            };
            match visited.entry((type_info.clone(), target_async)) {
                // Keep the full diagnostic path, including the prefix before the cycle.
                BTreeMapEntry::Occupied(state) if *state.get() => return true,
                BTreeMapEntry::Occupied(_) => (),
                BTreeMapEntry::Vacant(state) => {
                    state.insert(true);
                    stack.push(CycleFrame {
                        key: type_info.clone(),
                        is_async: target_async,
                        dependencies: dependencies.iter(),
                    });
                }
            }
        }
        false
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Selection<Data> {
    ByType,
    Indexed(Option<Data>),
}

impl<Data> Selection<Data> {
    pub(crate) fn or_lookup(self, lookup: impl FnOnce() -> Option<Data>) -> Option<Data> {
        match self {
            Self::ByType => lookup(),
            Self::Indexed(data) => data,
        }
    }
}
