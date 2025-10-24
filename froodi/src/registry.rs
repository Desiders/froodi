use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    vec::Vec,
};
use core::any::TypeId;

use crate::{
    dependency::Dependency,
    errors::DFSErrorKind,
    finalizer::BoxedCloneFinalizer,
    instantiator::BoxedCloneInstantiator,
    scope::{ScopeData, ScopeDataWithChildScopesData},
    Config, InstantiateErrorKind, ResolveErrorKind,
};

#[derive(Clone)]
pub(crate) struct InstantiatorData {
    pub(crate) instantiator: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,
    pub(crate) dependencies: BTreeSet<Dependency>,
    pub(crate) finalizer: Option<BoxedCloneFinalizer>,
    pub(crate) config: Config,
    pub(crate) scope_data: ScopeData,
}

#[derive(Clone, Default)]
pub struct Registry {
    pub(crate) entries: BTreeMap<TypeId, InstantiatorData>,
    pub(crate) scopes_data: BTreeSet<ScopeData>,
}

impl Registry {
    #[inline]
    pub(crate) fn get(&self, type_id: &TypeId) -> Option<&InstantiatorData> {
        self.entries.get(&type_id)
    }

    #[inline]
    pub(crate) fn get_scope_with_child_scopes(&self) -> ScopeDataWithChildScopesData {
        ScopeDataWithChildScopesData::new(self.scopes_data.clone().into_iter().collect())
    }

    pub(crate) fn dfs_detect<'a>(&'a self) -> Result<(), DFSErrorKind> {
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
    (
        $( scope($scope:expr) [ $( $entries:tt )* ] ),* $(,)?
    ) => {{
        #[allow(unused_mut)]
        let mut entries = ::alloc::collections::BTreeMap::new();
        #[allow(unused_mut)]
        let mut scopes_data = ::alloc::collections::BTreeSet::new();
        $(
            let scope_data = $scope.into();
            scopes_data.insert(scope_data);
            entries.insert(
                ::core::any::TypeId::of::<$crate::Container>(),
                $crate::registry::InstantiatorData {
                    instantiator: $crate::instantiator::boxed_container_instantiator(),
                    dependencies: ::alloc::collections::BTreeSet::new(),
                    finalizer: None,
                    config: $crate::Config { cache_provides: true },
                    scope_data,
                },
            );
            entries.extend($crate::registry_internal! { @entries scope($scope) [ $( $entries )* ] });
        )*
        $crate::registry::Registry { entries, scopes_data }
    }};

    (
        $( scope($scope:expr) [ $( $entries:tt )* ], )*
        extend($( $registries:expr ),+ $(,)?) $(,)?
    ) => {{
        let mut registry = registry! { $( scope($scope) [ $( $entries )* ] ),* };
        $(
            registry.entries.extend($registries.entries);
            registry.scopes_data.extend($registries.scopes_data);
        )*
        registry
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! registry_internal {
    (@entries scope($scope:expr) [ $( provide( $($entry:tt)+ ) ),* $(,)? ]) => {{
        ::alloc::collections::BTreeMap::from_iter([ $( $crate::registry_internal! { @entry scope($scope), $($entry)+ } ),* ])
    }};

    (@entry scope($scope:expr), $inst:expr) => {{
        $crate::registry_internal! { @entry_with_options scope($scope), $inst, config = None, finalizer = None::<fn(_)> }
    }};
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr) => {{
        $crate::registry_internal! { @entry_with_options scope($scope), $inst, config = Some($cfg), finalizer = None::<fn(_)> }
    }};
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr) => {{
        $crate::registry_internal! { @entry_with_options scope($scope), $inst, config = None, finalizer = Some($fin) }
    }};
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr, finalizer = $fin:expr) => {{
        $crate::registry_internal! { @entry_with_options scope($scope), $inst, config = Some($cfg), finalizer = Some($fin) }
    }};
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr, config = $cfg:expr) => {{
        $crate::registry_internal! { @entry_with_options scope($scope), $inst, config = Some($cfg), finalizer = Some($fin) }
    }};

    (@entry_with_options scope($scope:expr), $inst:expr, config = $cfg:expr, finalizer = $fin:expr) => {{
        #[inline]
        fn impl_<Inst, Deps, Fin>(inst: Inst, fin: Option<Fin>) -> (core::any::TypeId, $crate::registry::InstantiatorData)
        where
            Inst: $crate::instantiator::Instantiator<Deps, Error = $crate::InstantiateErrorKind>
                + $crate::utils::thread_safety::SendSafety
                + $crate::utils::thread_safety::SyncSafety,
            Inst::Provides:
                  $crate::utils::thread_safety::SendSafety
                + $crate::utils::thread_safety::SyncSafety,
            Deps: $crate::dependency_resolver::DependencyResolver<Error = $crate::ResolveErrorKind>,
            Fin:  $crate::finalizer::Finalizer<Inst::Provides>
                + $crate::utils::thread_safety::SendSafety
                + $crate::utils::thread_safety::SyncSafety,
        {
            (::core::any::TypeId::of::<Inst::Provides>(), $crate::registry::InstantiatorData {
                dependencies: Inst::dependencies(),
                instantiator: $crate::instantiator::boxed_instantiator(inst),
                finalizer: match fin {
                    Some(finalizer) => Some($crate::finalizer::boxed_finalizer_factory(finalizer)),
                    None => None,
                },
                config: match $cfg {
                    Some(config) => config,
                    None => $crate::Config::default(),
                },
                scope_data: $scope.into(),
            })
        }
        impl_($inst, $fin)
    }};
}

#[cfg(test)]
mod tests {
    extern crate std;

    use core::any::TypeId;

    use alloc::{
        format,
        string::{String, ToString as _},
    };
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
                provide(inst_b , config = Config::default() , finalizer = fin_b)
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

        assert_eq!(registry.entries.len(), 6);
    }
}
