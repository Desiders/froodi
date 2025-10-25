use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    vec::Vec,
};
use core::any::TypeId;

use crate::{
    dependency::Dependency,
    errors::DFSErrorKind,
    finalizer::BoxedCloneFinalizer,
    instantiator::{boxed_container_instantiator, BoxedCloneInstantiator},
    scope::{ScopeData, ScopeDataWithChildScopesData},
    Config, Container, DefaultScope, InstantiateErrorKind, ResolveErrorKind, Scope, Scopes,
};

#[derive(Clone)]
pub struct InstantiatorData {
    pub(crate) instantiator: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,
    pub(crate) dependencies: BTreeSet<Dependency>,
    pub(crate) finalizer: Option<BoxedCloneFinalizer>,
    pub(crate) config: Config,
    pub(crate) scope_data: ScopeData,
}

#[derive(Clone)]
pub struct Registry {
    pub(crate) entries: BTreeMap<TypeId, InstantiatorData>,
    pub(crate) scopes_data: Vec<ScopeData>,
}

impl Registry {
    pub(crate) fn new<T, S, const N: usize>(mut entries: BTreeMap<TypeId, InstantiatorData>) -> Self
    where
        S: Scope,
        T: Scopes<N, Scope = S>,
    {
        const DEPENDENCIES: BTreeSet<Dependency> = BTreeSet::new();

        let mut scopes_data = Vec::with_capacity(N);
        for scope in T::all() {
            let scope_data = scope.into();

            entries.insert(
                TypeId::of::<Container>(),
                InstantiatorData {
                    instantiator: boxed_container_instantiator(),
                    dependencies: DEPENDENCIES,
                    finalizer: None,
                    config: Config { cache_provides: true },
                    scope_data,
                },
            );
            scopes_data.push(scope_data);
        }
        Self { entries, scopes_data }
    }

    #[inline]
    pub fn new_with_default_entries() -> Self {
        Self::new::<DefaultScope, DefaultScope, 6>(BTreeMap::new())
    }

    #[inline]
    pub fn merge(&mut self, other: Self) {
        self.entries.extend(other.entries);
    }
}

impl Registry {
    #[inline]
    pub(crate) fn get(&self, type_id: &TypeId) -> Option<&InstantiatorData> {
        self.entries.get(type_id)
    }

    #[inline]
    pub(crate) fn get_scope_with_child_scopes(&self) -> ScopeDataWithChildScopesData {
        ScopeDataWithChildScopesData::new_with_sort(self.scopes_data.clone().into_iter().collect())
    }

    pub(crate) fn dfs_detect(&self) -> Result<(), DFSErrorKind> {
        let mut visited = BTreeSet::new();
        let mut stack = Vec::new();

        for (type_id, InstantiatorData { dependencies, .. }) in &self.entries {
            if self.dfs_visit(type_id, dependencies, &mut visited, &mut stack) {
                return Err(DFSErrorKind::CyclicDependency {
                    graph: (stack.remove(0), stack.into_boxed_slice()),
                });
            }
        }
        Ok(())
    }

    fn dfs_visit<'a>(
        &self,
        type_id: &TypeId,
        dependencies: &BTreeSet<Dependency>,
        visited: &'a mut BTreeSet<TypeId>,
        stack: &'a mut Vec<TypeId>,
    ) -> bool {
        if visited.contains(type_id) {
            return false;
        }
        if stack.contains(type_id) {
            return true;
        }
        stack.push(*type_id);

        for Dependency { type_id } in dependencies {
            if let Some(InstantiatorData { dependencies, .. }) = self.entries.get(type_id) {
                if self.dfs_visit(type_id, dependencies, visited, stack) {
                    return true;
                }
            }
        }

        stack.pop();
        visited.insert(*type_id);
        false
    }
}

#[macro_export]
macro_rules! registry {
    () => {{
        $crate::Registry::new_with_default_entries()
    }};

    (scope($scope:expr) [ $($entries:tt)* ] $(, $($rest:tt)+)?) => {{
        #[allow(unused_mut)]
        let mut registry = $crate::macros_utils::sync::build_registry([
            ($scope, $crate::registry_internal! { @entries scope($scope) [ $( $entries )* ] })
        ]);
        $(
            registry.merge($crate::registry! { $($rest)+ });
        )*
        registry
    }};

    (scope($scope:expr) [ $($entries:tt)* ] $(,)?) => {{
        $crate::macros_utils::sync::build_registry([
            ($scope, $crate::registry_internal! { @entries scope($scope) [ $( $entries )* ] })
        ])
    }};

    ($( extend( $($registries:expr),* $(,)? ) ),* $(,)?) => {{
        let mut registry = $crate::Registry::new_with_default_entries();
        $(
            $(
                registry.merge($registries);
            )+
        )*
        registry
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! registry_internal {
    (@entries scope($scope:expr) [ $( provide( $($entry:tt)+ ) ),* $(,)? ]) => {{
        $crate::macros_utils::aliases::Vec::from_iter([$( $crate::registry_internal! { @entry scope($scope), $($entry)+ } ),*])
    }};
    (@entry scope($scope:expr), $inst:expr $(,)?) => {{
        $crate::macros_utils::sync::make_entry($scope, $inst, None, None::<$crate::macros_utils::sync::FinDummy<_>>)
    }};
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr $(,)?) => {{
        $crate::macros_utils::sync::make_entry($scope, $inst, Some($cfg), None::<$crate::macros_utils::sync::FinDummy<_>>)
    }};
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr $(,)?) => {{
        $crate::macros_utils::sync::make_entry($scope, $inst, None, Some($fin))
    }};
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr, finalizer = $fin:expr $(,)?) => {{
        $crate::macros_utils::sync::make_entry($scope, $inst, Some($cfg), Some($fin))
    }};
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr, config = $cfg:expr $(,)?) => {{
        $crate::macros_utils::sync::make_entry($scope, $inst, Some($cfg), Some($fin))
    }};
}

#[cfg(test)]
mod tests {
    extern crate std;

    use alloc::{
        format,
        string::{String, ToString as _},
    };
    use core::any::TypeId;
    use tracing_test::traced_test;

    use crate::{utils::thread_safety::RcThreadSafety, Config, DefaultScope, Inject, InjectTransient, InstantiateErrorKind};

    fn inst_a() -> Result<(), InstantiateErrorKind> {
        Ok(())
    }
    fn inst_b() -> Result<((), ()), InstantiateErrorKind> {
        Ok(((), ()))
    }
    fn inst_c() -> Result<((), (), ()), InstantiateErrorKind> {
        Ok(((), (), ()))
    }
    fn inst_d() -> Result<((), (), (), ()), InstantiateErrorKind> {
        Ok(((), (), (), ()))
    }
    fn inst_e() -> Result<((), (), (), (), ()), InstantiateErrorKind> {
        Ok(((), (), (), (), ()))
    }
    fn inst_f() -> Result<((), (), (), (), (), ()), InstantiateErrorKind> {
        Ok(((), (), (), (), (), ()))
    }

    fn fin_a(_val: RcThreadSafety<()>) {}
    fn fin_b(_val: RcThreadSafety<((), ())>) {}
    fn fin_c(_val: RcThreadSafety<((), (), ())>) {}
    fn fin_d(_val: RcThreadSafety<((), (), (), ())>) {}
    fn fin_e(_val: RcThreadSafety<((), (), (), (), ())>) {}
    fn fin_f(_val: RcThreadSafety<((), (), (), (), (), ())>) {}

    #[test]
    #[traced_test]
    fn test_registry_mixed_entries() {
        registry! {
            scope(DefaultScope::App) [
                provide(|| Ok(())), // вместо функции замыкание
                provide(|Inject(_): Inject<()>| Ok(((), ()))),  // вместо функции замыкание + инъекция значения из фабрики что возвращает `()`, т.е. верхней
                provide(inst_c, config = Config::default()),
                provide(inst_d, finalizer = fin_d),
                provide(inst_e, config = Config::default(), finalizer = fin_e),
                provide(inst_f, finalizer = fin_f, config = Config::default()),
            ]
        };
    }

    #[test]
    #[traced_test]
    fn test_entry_simple_ident() {
        registry_internal! { @entries scope(DefaultScope::App) [ provide(inst_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_tuple_single() {
        registry_internal! { @entries scope(DefaultScope::App) [ provide(inst_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_config() {
        registry_internal! { @entries scope(DefaultScope::App) [ provide(inst_a, config = Config::default()) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_finalizer() {
        registry_internal! { @entries scope(DefaultScope::App) [ provide(inst_a, finalizer = fin_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_config_and_finalizer() {
        registry_internal! { @entries scope(DefaultScope::App) [ provide(inst_a, config = Config::default(), finalizer = fin_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_finalizer_and_config_swapped() {
        registry_internal! { @entries scope(DefaultScope::App) [ provide(inst_a, finalizer = fin_a, config = Config::default()) ] };
    }

    #[test]
    #[traced_test]
    fn test_multiple_entries() {
        registry_internal! {
            @entries scope(DefaultScope::App) [
                provide(inst_a),
                provide(inst_b),
                provide(inst_c, config = Config::default(), finalizer = fin_c),
                provide(inst_d, finalizer = fin_d),
                provide(inst_e, config = Config::default(), finalizer = fin_e),
            ]
        };
    }

    #[test]
    #[traced_test]
    fn test_trailing_comma_and_spaces() {
        registry_internal! {
            @entries scope(DefaultScope::App) [
                provide(inst_a, config = Config::default(), finalizer = fin_a),
            ]
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_single_scope_basic() {
        registry! {
            scope(DefaultScope::App) [
                provide(inst_a),
                provide(inst_b),
                provide(inst_c, config = Config::default()),
                provide(inst_d, finalizer = fin_d),
                provide(inst_e, config = Config::default(), finalizer = fin_e),
            ],
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_multiple_scopes() {
        registry! {
            scope(DefaultScope::App) [
                provide(inst_a),
                provide(inst_b),
            ],
            scope(DefaultScope::Request) [
                provide(inst_c, config = Config::default()),
                provide(inst_d, finalizer = fin_d),
            ],
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_empty_scope() {
        registry! {
            scope(DefaultScope::App) []
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_trailing_commas_and_spacing() {
        registry! {
            scope(DefaultScope::App)[
                provide(inst_a),
                provide(inst_b , config = Config::default() , finalizer = fin_b ,)
            ]
            , scope(DefaultScope::Request)[ provide(inst_c) , ]
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_get() {
        let registry = registry! {
            scope(DefaultScope::App) [],
            scope(DefaultScope::Session) [provide(inst_a), provide(inst_b), provide(inst_c)],
            scope(DefaultScope::Request) [provide(inst_d), provide(inst_e), provide(inst_f)],
        };

        assert!(registry.get(&TypeId::of::<()>()).is_some());
        assert!(registry.get(&TypeId::of::<((), ())>()).is_some());
        assert!(registry.get(&TypeId::of::<((), (), ())>()).is_some());
        assert!(registry.get(&TypeId::of::<((), (), (), ())>()).is_some());
        assert!(registry.get(&TypeId::of::<((), (), (), (), ())>()).is_some());
        assert!(registry.get(&TypeId::of::<((), (), (), (), (), ())>()).is_some());
        assert!(registry.get(&TypeId::of::<((), (), (), (), (), (), ())>()).is_none());
    }

    #[test]
    #[traced_test]
    fn test_registry_dfs_detect_ok() {
        struct A;
        struct B(A);
        struct C(B, A);

        let registry = registry! {
            scope(DefaultScope::App) [
                provide(|| Ok(A)),
            ],
            scope(DefaultScope::Session) [
                provide(|InjectTransient(a): InjectTransient<A>| Ok(B(a))),
            ],
            scope(DefaultScope::Request) [
                provide(|InjectTransient(b): InjectTransient<B>, InjectTransient(a): InjectTransient<A>| Ok(C(b, a))),
            ],
        };
        registry.dfs_detect().unwrap();
    }

    #[test]
    #[traced_test]
    fn test_registry_dfs_detect_single() {
        struct A;

        let registry = registry! {
            scope(DefaultScope::App) [
                provide(|InjectTransient(_): InjectTransient<A>| Ok(A)),
            ],
        };
        registry.dfs_detect().unwrap_err();
    }

    #[test]
    #[traced_test]
    fn test_registry_dfs_detect_many() {
        struct A;
        struct B;

        let registry = registry! {
            scope(DefaultScope::App) [
                provide(|InjectTransient(_): InjectTransient<B>| Ok(A)),
            ],
            scope(DefaultScope::Session) [
                provide(|InjectTransient(_): InjectTransient<A>| Ok(B)),
            ],
        };
        registry.dfs_detect().unwrap_err();
    }

    #[test]
    #[traced_test]
    fn registry_extend_entries() {
        let registry = registry! {
            scope(DefaultScope::Session) [provide(inst_a)],
            extend(
                registry! {
                    scope(DefaultScope::App) [provide(inst_b)],
                    extend(
                        registry! {
                            scope(DefaultScope::Session) [provide(inst_c)],
                        },
                        registry! {
                            scope(DefaultScope::Request) [provide(inst_d)],
                        },
                    ),
                },
                registry! {
                    scope(DefaultScope::Session) [provide(inst_e)],
                    extend(
                        registry! {
                            scope(DefaultScope::Session) [provide(inst_f)],
                        },
                    ),
                },
            ),
        };

        assert_eq!(registry.entries.len(), 7);
    }
}
