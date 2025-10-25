use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    vec::Vec,
};
use core::any::TypeId;

use crate::{
    async_impl::{
        finalizer::BoxedCloneFinalizer,
        instantiator::{boxed_container_instantiator, BoxedCloneInstantiator},
        Container,
    },
    dependency::Dependency,
    errors::DFSErrorKind,
    scope::{ScopeData, ScopeDataWithChildScopesData},
    Config, DefaultScope, InstantiateErrorKind, Registry as SyncRegistry, ResolveErrorKind, Scope, Scopes,
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

    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::new::<DefaultScope, DefaultScope, 6>(BTreeMap::new())
    }
}

#[derive(Clone)]
pub struct RegistryWithSync {
    pub registry: Registry,
    pub sync: SyncRegistry,
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
macro_rules! async_registry {
    () => {{
        $crate::async_impl::registry::RegistryWithSync {
            registry: $crate::async_impl::Registry::default(),
            sync: $crate::Registry::default(),
        }
    }};

    (
        $( scope($scope:expr) [ $( $entries:tt )* ] ),+ $(,)?
    ) => {{
        $crate::async_impl::RegistryWithSync {
            registry: $crate::macros_utils::async_impl::build_registry([
                $(
                    ($scope, $crate::async_registry_internal! { @entries scope($scope) [ $( $entries )* ] })
                ),*
            ]),
            sync: $crate::Registry::default(),
        }
    }};

    (
        $( scope($scope:expr) [ $( $entries:tt )* ], )*
        sync = $sync_registry:expr $(,)?
    ) => {{
        let mut registry_with_sync = async_registry! { $( scope($scope) [ $( $entries )* ] ),* };
        registry_with_sync.sync = $sync_registry;
        registry_with_sync
    }};

    (
        $( scope($scope:expr) [ $( $entries:tt )* ], )*
        extend($( $registries:expr ),+ $(,)?)
        $(, sync = $sync_registry:expr )? $(,)?
    ) => {{
        let mut registry_with_sync = async_registry! {
            $( scope($scope) [ $( $entries )* ], )*
            $( sync = $sync_registry )?
        };
        $(
            registry_with_sync.registry.entries.extend($registries.registry.entries);
            registry_with_sync.registry.scopes_data.extend($registries.registry.scopes_data);
        )*
        registry_with_sync
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! async_registry_internal {
    (@entries scope($scope:expr) [ $( provide( $($entry:tt)+ ) ),* $(,)? ]) => {{
        $crate::macros_utils::aliases::Vec::from_iter([$( $crate::async_registry_internal! { @entry scope($scope), $($entry)+ } ),*])
    }};
    (@entry scope($scope:expr), $inst:expr $(,)?) => {{
        $crate::macros_utils::async_impl::make_entry($scope, $inst, None, None::<$crate::macros_utils::async_impl::FinDummy<_>>)
    }};
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr $(,)?) => {{
        #[cfg(feature = "thread_safe")]
        type Fin<T> = fn(T) -> ::core::pin::Pin<::alloc::boxed::Box<dyn ::core::future::Future<Output = ()> + Send>>;
        #[cfg(not(feature = "thread_safe"))]
        type Fin<T> = fn(T) -> ::core::pin::Pin<::alloc::boxed::Box<dyn ::core::future::Future<Output = ()>>>;

        $crate::macros_utils::async_impl::make_entry($scope, $inst, Some($cfg), None::<Fin<_>>)
    }};
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr $(,)?) => {{
        $crate::macros_utils::async_impl::make_entry($scope, $inst, None, Some($fin))
    }};
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr, finalizer = $fin:expr $(,)?) => {{
        $crate::macros_utils::async_impl::make_entry($scope, $inst, Some($cfg), Some($fin))
    }};
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr, config = $cfg:expr $(,)?) => {{
        $crate::macros_utils::async_impl::make_entry($scope, $inst, Some($cfg), Some($fin))
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

    use crate::{
        async_impl::registry::RegistryWithSync, registry, utils::thread_safety::RcThreadSafety, Config, DefaultScope, Inject,
        InjectTransient, InstantiateErrorKind,
    };

    async fn inst_a() -> Result<(), InstantiateErrorKind> {
        Ok(())
    }
    async fn inst_b() -> Result<((), ()), InstantiateErrorKind> {
        Ok(((), ()))
    }
    async fn inst_c() -> Result<((), (), ()), InstantiateErrorKind> {
        Ok(((), (), ()))
    }
    async fn inst_d() -> Result<((), (), (), ()), InstantiateErrorKind> {
        Ok(((), (), (), ()))
    }
    async fn inst_e() -> Result<((), (), (), (), ()), InstantiateErrorKind> {
        Ok(((), (), (), (), ()))
    }
    async fn inst_f() -> Result<((), (), (), (), (), ()), InstantiateErrorKind> {
        Ok(((), (), (), (), (), ()))
    }

    async fn fin_a(_val: RcThreadSafety<()>) {}
    async fn fin_b(_val: RcThreadSafety<((), ())>) {}
    async fn fin_c(_val: RcThreadSafety<((), (), ())>) {}
    async fn fin_d(_val: RcThreadSafety<((), (), (), ())>) {}
    async fn fin_e(_val: RcThreadSafety<((), (), (), (), ())>) {}
    async fn fin_f(_val: RcThreadSafety<((), (), (), (), (), ())>) {}

    #[test]
    #[traced_test]
    fn test_registry_mixed_entries() {
        async_registry! {
            scope(DefaultScope::App) [
                provide(async || Ok(())), // вместо функции замыкание
                provide(async |Inject(_): Inject<()>| Ok(((), ()))),  // вместо функции замыкание + инъекция значения из фабрики что возвращает `()`, т.е. верхней
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
        async_registry_internal! { @entries scope(DefaultScope::App) [ provide(inst_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_tuple_single() {
        async_registry_internal! { @entries scope(DefaultScope::App) [ provide(inst_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_config() {
        async_registry_internal! { @entries scope(DefaultScope::App) [ provide(inst_a, config = Config::default()) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_finalizer() {
        async_registry_internal! { @entries scope(DefaultScope::App) [ provide(inst_a, finalizer = fin_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_config_and_finalizer() {
        async_registry_internal! { @entries scope(DefaultScope::App) [ provide(inst_a, config = Config::default(), finalizer = fin_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_finalizer_and_config_swapped() {
        async_registry_internal! { @entries scope(DefaultScope::App) [ provide(inst_a, finalizer = fin_a, config = Config::default()) ] };
    }

    #[test]
    #[traced_test]
    fn test_multiple_entries() {
        async_registry_internal! {
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
        async_registry_internal! {
            @entries scope(DefaultScope::App) [
                provide(inst_a, config = Config::default(), finalizer = fin_a),
            ]
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_single_scope_basic() {
        async_registry! {
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
        async_registry! {
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
        async_registry! {
            scope(DefaultScope::App) []
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_trailing_commas_and_spacing() {
        async_registry! {
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
        let RegistryWithSync { registry, .. } = async_registry! {
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

        let RegistryWithSync { registry, .. } = async_registry! {
            scope(DefaultScope::App) [
                provide(async || Ok(A)),
            ],
            scope(DefaultScope::Session) [
                provide(async |InjectTransient(a): InjectTransient<A>| Ok(B(a))),
            ],
            scope(DefaultScope::Request) [
                provide(async |InjectTransient(b): InjectTransient<B>, InjectTransient(a): InjectTransient<A>| Ok(C(b, a))),
            ],
        };
        registry.dfs_detect().unwrap();
    }

    #[test]
    #[traced_test]
    fn test_registry_dfs_detect_single() {
        struct A;

        let RegistryWithSync { registry, .. } = async_registry! {
            scope(DefaultScope::App) [
                provide(async |InjectTransient(_): InjectTransient<A>| Ok(A)),
            ],
        };
        registry.dfs_detect().unwrap_err();
    }

    #[test]
    #[traced_test]
    fn test_registry_dfs_detect_many() {
        struct A;
        struct B;

        let RegistryWithSync { registry, .. } = async_registry! {
            scope(DefaultScope::App) [
                provide(async |InjectTransient(_): InjectTransient<B>| Ok(A)),
            ],
            scope(DefaultScope::Session) [
                provide(async |InjectTransient(_): InjectTransient<A>| Ok(B)),
            ],
        };
        registry.dfs_detect().unwrap_err();
    }

    #[test]
    #[traced_test]
    fn registry_extend_entries() {
        let RegistryWithSync { registry, .. } = async_registry! {
            scope(DefaultScope::Session) [provide(inst_a)],
            extend(
                async_registry! {
                    scope(DefaultScope::App) [provide(inst_b)],
                    extend(
                        async_registry! {
                            scope(DefaultScope::Session) [provide(inst_c)],
                        },
                        async_registry! {
                            scope(DefaultScope::Request) [provide(inst_d)],
                        },
                    ),
                },
                async_registry! {
                    scope(DefaultScope::Session) [provide(inst_e)],
                    extend(
                        async_registry! {
                            scope(DefaultScope::Session) [provide(inst_f)],
                        },
                    ),
                },
            ),
        };

        assert_eq!(registry.entries.len(), 7);
    }

    #[test]
    #[traced_test]
    fn test_registry_with_sync() {
        async_registry! {
            scope(DefaultScope::App) [
                provide(inst_a),
            ],
            sync = registry! {
                scope(DefaultScope::App) []
            },
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_with_sync_and_extend() {
        async_registry! {
            scope(DefaultScope::App) [
                provide(inst_a),
            ],
            extend(
                async_registry! {
                    scope(DefaultScope::Session) [
                        provide(inst_b),
                    ],
                },
            ),
            sync = registry! {
                scope(DefaultScope::App) []
            },
        };
    }
}
