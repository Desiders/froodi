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
pub struct InstantiatorData {
    pub(crate) instantiator: RegistrationInstantiator,
    pub(crate) dependencies: BTreeSet<Dependency>,
    pub(crate) finalizer: Option<BoxedCloneFinalizer>,
    pub(crate) config: Config,
    pub(crate) scope_data: ScopeData,
}

#[derive(Clone, Default)]
pub struct Registry {
    pub(crate) entries: BTreeMap<TypeInfo, InstantiatorData>,
    pub(crate) scopes_data: Vec<ScopeData>,
    pub(crate) indexed: Vec<(TypeInfo, Option<InstantiatorData>)>,
}

impl Registry {
    #[allow(clippy::similar_names)]
    pub(crate) fn new<T, S, const N: usize>(mut entries: BTreeMap<TypeInfo, InstantiatorData>) -> Self
    where
        S: Scope,
        T: Scopes<N, Scope = S>,
    {
        let (scope, child_scopes) = T::all();

        let mut scopes_data = Vec::with_capacity(N + 1);
        let scope_data = scope.into();

        entries.insert(
            TypeInfo::new::<Container>("Container"),
            InstantiatorData {
                instantiator: boxed_container_instantiator().into(),
                dependencies: EMPTY_DEPENDENCIES,
                finalizer: None,
                // Caching the container in its own cache creates an cycle
                // that prevents `Drop`/`close` from ever running
                config: Config { cache_provides: false },
                scope_data,
            },
        );

        scopes_data.push(scope_data);
        for scope in child_scopes {
            scopes_data.push(scope.into());
        }

        Self {
            entries,
            scopes_data,
            indexed: Vec::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn new_with_default_entries() -> Self {
        Self::new::<DefaultScope, DefaultScope, 5>(BTreeMap::new())
    }
}

impl Registry {
    #[inline]
    pub(crate) fn get(&self, type_info: &TypeInfo) -> Option<&InstantiatorData> {
        self.entries.get(type_info)
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

        for (type_info, InstantiatorData { dependencies, .. }) in &self.entries {
            if self.dfs_visit(type_info, dependencies, &mut visited, &mut stack) {
                return Err(ValidationErrorKind::CyclicDependency {
                    graph: (stack.remove(0), stack.into_boxed_slice()),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn detect_unreachable_scopes(&self) -> Result<(), ValidationErrorKind> {
        for (
            type_info,
            InstantiatorData {
                dependencies, scope_data, ..
            },
        ) in &self.entries
        {
            for Dependency { type_info: dependency } in dependencies {
                if let Some(InstantiatorData {
                    scope_data: dependency_scope,
                    ..
                }) = self.entries.get(dependency)
                {
                    if !scope_data.can_access(dependency_scope) {
                        return Err(ValidationErrorKind::UnreachableDependency {
                            dependent: type_info.clone(),
                            dependent_scope: *scope_data,
                            dependency: dependency.clone(),
                            dependency_scope: *dependency_scope,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn dfs_visit<'a>(
        &self,
        type_info: &TypeInfo,
        dependencies: &BTreeSet<Dependency>,
        visited: &'a mut BTreeSet<TypeInfo>,
        stack: &'a mut Vec<TypeInfo>,
    ) -> bool {
        if visited.contains(type_info) {
            return false;
        }
        if stack.contains(type_info) {
            return true;
        }
        stack.push(type_info.clone());

        for Dependency { type_info } in dependencies {
            if let Some(InstantiatorData { dependencies, .. }) = self.entries.get(type_info) {
                if self.dfs_visit(type_info, dependencies, visited, stack) {
                    return true;
                }
            }
        }

        stack.pop();
        visited.insert(type_info.clone());
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
