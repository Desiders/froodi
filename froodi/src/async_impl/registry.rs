use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    vec::Vec,
};

use crate::{
    any::TypeInfo,
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

#[derive(Clone, Default)]
pub struct Registry {
    pub(crate) entries: BTreeMap<TypeInfo, InstantiatorData>,
    pub(crate) scopes_data: Vec<ScopeData>,
}

impl Registry {
    #[allow(clippy::similar_names)]
    pub(crate) fn new<T, S, const N: usize>(mut entries: BTreeMap<TypeInfo, InstantiatorData>) -> Self
    where
        S: Scope,
        T: Scopes<N, Scope = S>,
    {
        const DEPENDENCIES: BTreeSet<Dependency> = BTreeSet::new();

        let mut scopes_data = Vec::with_capacity(N);
        for scope in T::all() {
            let scope_data = scope.into();

            entries.insert(
                TypeInfo::of::<Container>(),
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
    #[must_use]
    pub fn new_with_default_entries() -> Self {
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
    pub(crate) fn get(&self, type_info: &TypeInfo) -> Option<&InstantiatorData> {
        self.entries.get(type_info)
    }

    #[inline]
    pub(crate) fn get_scope_with_child_scopes(&self) -> ScopeDataWithChildScopesData {
        ScopeDataWithChildScopesData::new_with_sort(self.scopes_data.clone().into_iter().collect())
    }

    pub(crate) fn dfs_detect(&self) -> Result<(), DFSErrorKind> {
        let mut visited = BTreeSet::new();
        let mut stack = Vec::new();

        for (type_info, InstantiatorData { dependencies, .. }) in &self.entries {
            if self.dfs_visit(type_info, dependencies, &mut visited, &mut stack) {
                return Err(DFSErrorKind::CyclicDependency {
                    graph: (stack.remove(0), stack.into_boxed_slice()),
                });
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
        stack.push(*type_info);

        for Dependency { type_info } in dependencies {
            if let Some(InstantiatorData { dependencies, .. }) = self.entries.get(type_info) {
                if self.dfs_visit(type_info, dependencies, visited, stack) {
                    return true;
                }
            }
        }

        stack.pop();
        visited.insert(*type_info);
        false
    }
}

/// The `async_registry!` macro is used to create an **asynchronous dependency registry**
/// with flexible configuration and composition options.
///
/// ### `provide` syntax
///
/// Each `provide` item defines a single asynchronous dependency registration.
/// The following forms are supported:
///
/// ```no_code
/// provide(inst)                             // async factory only
/// provide(inst, config = Config::default()) // with configuration
/// provide(inst, finalizer = fin)            // with async finalizer
/// provide(inst, config = Config::default(), finalizer = fin) // with both parameters
/// provide(inst, finalizer = fin, config = Config::default()) // order doesn’t matter
/// ```
///
/// Parameters:
/// - `config` *(optional)* — configuration object.
/// - `finalizer` *(optional)* — asynchronous function called when the dependency is finalized.
///
/// ## Usage patterns
///
/// ### 1. Single `scope`
/// ```rust
/// use froodi::{async_registry, InstantiateErrorKind, DefaultScope::*};
///
/// async fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// async_registry! {
///     scope(App) [
///         provide(inst),
///     ]
/// };
/// ```
///
/// ### 2. Multiple `scope`
/// ```rust
/// use froodi::{async_registry, InstantiateErrorKind, DefaultScope::*};
///
/// async fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// async_registry! {
///     scope(App) [ provide(inst) ],
///     scope(Session) [ provide(inst) ],
/// };
/// ```
///
/// ### 3. Single `provide`
/// ```rust
/// use froodi::{async_registry, InstantiateErrorKind, DefaultScope::*};
///
/// async fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// async_registry! {
///     provide(App, inst)
/// };
/// ```
///
/// ### 4. Multiple `provide`
/// ```rust
/// use froodi::{async_registry, InstantiateErrorKind, DefaultScope::*};
///
/// async fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// async_registry! {
///     provide(App, inst),
///     provide(Session, inst),
///     provide(Request, inst),
/// };
/// ```
///
/// ### 5. Combination of one or more `scope` and `provide`
/// ```rust
/// use froodi::{async_registry, InstantiateErrorKind, DefaultScope::*};
///
/// async fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// async_registry! {
///     scope(App) [ provide(inst) ],
///     provide(Session, inst),
///     provide(Request, inst),
/// };
/// ```
///
/// ### 6. Using `extend` standalone
/// ```rust
/// use froodi::{async_registry, DefaultScope::*};
///
/// async_registry! {
///     extend(async_registry!())
/// };
/// ```
///
/// ### 7. Using `extend` together with a combination of `scope` and `provide`
/// ```rust
/// use froodi::{async_registry, InstantiateErrorKind, DefaultScope::*};
///
/// async fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// async_registry! {
///     scope(App) [ provide(inst) ],
///     provide(Session, inst),
///     extend(async_registry!(), async_registry!()),
/// };
/// ```
///
/// ### 8. Empty macro usage
/// ```rust
/// use froodi::async_registry;
///
/// let registry = async_registry!();
/// ```
/// Creates an asynchronous registry with default entries.
///
/// ### 9. Using `sync`
/// ```rust
/// use froodi::{async_registry, registry, InstantiateErrorKind, DefaultScope::*};
///
/// async fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// async_registry! {
///     scope(App) [ provide(inst) ],
///     provide(Session, inst),
///     extend(async_registry!(), async_registry!()),
///     sync = registry!(),
/// };
/// ```
/// The `sync` keyword allows linking an existing **synchronous registry** to the asynchronous one.
/// Only one `sync` container can be specified, and it must appear **at the end** of the macro.
#[macro_export]
macro_rules! async_registry {
    () => {{
        $crate::async_impl::RegistryWithSync {
            registry: $crate::async_impl::Registry::new_with_default_entries(),
            sync: $crate::Registry::new_with_default_entries(),
        }
    }};
    (scope($scope:expr) [ $($entries:tt)+ ], $($rest:tt)+) => {{

        $crate::utils::Merge::merge(
            $crate::macros_utils::async_impl::build_registry(($scope, $crate::async_registry_internal! { scope($scope) [ $($entries)+ ] })),
            $crate::async_registry_internal! { $($rest)+ }
        )
    }};
    (scope($scope:expr) [ $($entries:tt)+ ] $(,)?) => {{
        $crate::macros_utils::async_impl::build_registry(($scope, $crate::async_registry_internal! { scope($scope) [ $($entries)+ ] }))
    }};
    (provide($scope:expr, $($entry:tt)+), $($rest:tt)+) => {{
        $crate::utils::Merge::merge(
            $crate::macros_utils::async_impl::build_registry(($scope, $crate::async_registry_internal! { provide($scope, $($entry)+) })),
            $crate::async_registry_internal! { $($rest)+ }
        )
    }};
    (provide($scope:expr, $($entry:tt)+) $(,)?) => {{
        $crate::macros_utils::async_impl::build_registry(($scope, $crate::async_registry_internal! { provide($scope, $($entry)+) }))
    }};
    (extend($registry:expr $(, $registries:expr),* $(,)?), $($rest:tt)+) => {{
        #[allow(unused_mut)]
        let mut registry = $registry;
        $(
            registry = $crate::utils::Merge::merge(registry, $registries);
        )*
        $crate::utils::Merge::merge(registry, $crate::async_registry! { $($rest)+ }) as $crate::async_impl::RegistryWithSync
    }};
    (extend($registry:expr $(, $registries:expr),* $(,)?) $(,)?) => {{
        #[allow(unused_mut)]
        let mut registry = $registry;
        $(
            registry = $crate::utils::Merge::merge(registry, $registries);
        )*
        registry as $crate::async_impl::RegistryWithSync
    }};
    (sync = $sync_registry:expr $(,)?) => {{
        $crate::async_impl::RegistryWithSync {
            registry: $crate::async_impl::Registry::new_with_default_entries(),
            sync: $sync_registry,
        }
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! async_registry_internal {
    (scope($scope:expr) [ $($entries:tt)+ ], $($rest:tt)+) => {{
        $crate::macros_utils::aliases::hlist![
            $crate::async_registry_internal! { @entries_in_scope scope($scope) [ $($entries)+ ] },
            $crate::async_registry_internal! { $($rest)+ }
        ]
    }};
    (scope($scope:expr) [ $($entries:tt)+ ] $(,)?) => {{
        $crate::async_registry_internal! { @entries_in_scope scope($scope) [ $($entries)+ ] }
    }};
    (provide($scope:expr, $($entry:tt)+), $($rest:tt)+) => {{
        $crate::macros_utils::aliases::hlist![
            $crate::async_registry_internal! { @entries_with_scope provide($scope, $($entry)+) },
            $crate::async_registry_internal! { $($rest)+ }
        ]
    }};
    (provide($scope:expr, $($entry:tt)+) $(,)?) => {{
        $crate::async_registry_internal! { @entries_with_scope provide($scope, $($entry)*) }
    }};
    (extend($registry:expr $(, $registries:expr),* $(,)?), $($rest:tt)+) => {{
        #[allow(unused_mut)]
        let mut registry = $registry;
        $(
            registry = $crate::utils::Merge::merge(registry, $registries);
        )*
        $crate::macros_utils::types::RegistryKindOrEntry::Kind(
            $crate::macros_utils::types::RegistryKind::AsyncWithSync(
                $crate::utils::Merge::merge(registry, $crate::async_registry_internal! { $($rest)+ })
            )
        )
    }};
    (extend($registry:expr $(, $registries:expr),* $(,)?) $(,)?) => {{
        #[allow(unused_mut)]
        let mut registry = $registry;
        $(
            registry = $crate::utils::Merge::merge(registry, $registries);
        )*
        $crate::macros_utils::types::RegistryKindOrEntry::Kind(
            $crate::macros_utils::types::RegistryKind::AsyncWithSync(registry)
        )
    }};
    (sync = $sync_registry:expr $(,)?) => {{
        $crate::macros_utils::types::RegistryKindOrEntry::Kind(
            $crate::macros_utils::types::RegistryKind::Sync($sync_registry)
        )
    }};

    (@entries_with_scope $( provide($scope:expr, $($entry:tt)+) ),+ $(,)?) => {{
        $crate::macros_utils::aliases::hlist![$( $crate::async_registry_internal! { @entry scope($scope), $($entry)+ } ),+]
    }};
    (@entries_in_scope scope($scope:expr) [ $( provide($($entry:tt)+) ),+ $(,)? ]) => {{
        $crate::macros_utils::aliases::hlist![$( $crate::async_registry_internal! { @entry scope($scope), $($entry)+ } ),+]
    }};
    (@entry scope($scope:expr), $inst:expr $(,)?) => {{
        $crate::macros_utils::types::RegistryKindOrEntry::Entry(
            $crate::macros_utils::async_impl::make_entry($scope, $inst, None, None::<$crate::macros_utils::async_impl::FinDummy<_>>)
        )
    }};
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr $(,)?) => {{
        $crate::macros_utils::types::RegistryKindOrEntry::Entry(
            $crate::macros_utils::async_impl::make_entry($scope, $inst, Some($cfg), None::<$crate::macros_utils::async_impl::FinDummy<_>>)
        )
    }};
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr $(,)?) => {{
        $crate::macros_utils::types::RegistryKindOrEntry::Entry($crate::macros_utils::async_impl::make_entry($scope, $inst, None, Some($fin)))
    }};
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr, finalizer = $fin:expr $(,)?) => {{
        $crate::macros_utils::types::RegistryKindOrEntry::Entry($crate::macros_utils::async_impl::make_entry($scope, $inst, Some($cfg), Some($fin)))
    }};
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr, config = $cfg:expr $(,)?) => {{
        $crate::macros_utils::types::RegistryKindOrEntry::Entry($crate::macros_utils::async_impl::make_entry($scope, $inst, Some($cfg), Some($fin)))
    }};
}

#[cfg(test)]
mod tests {
    extern crate std;

    use alloc::{
        format,
        string::{String, ToString as _},
    };
    use tracing_test::traced_test;

    use crate::{
        any::TypeInfo, async_impl::registry::RegistryWithSync, registry, utils::thread_safety::RcThreadSafety, Config, DefaultScope,
        Inject, InjectTransient, InstantiateErrorKind,
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
        let registry_a = async_registry! {
            provide(DefaultScope::Request, inst_a),
            scope(DefaultScope::App) [
                provide(async || Ok(())),
                provide(async |Inject(_): Inject<()>| Ok(((), ()))),
                provide(inst_c, config = Config::default()),
                provide(inst_d, finalizer = fin_d),
                provide(inst_e, config = Config::default(), finalizer = fin_e),
                provide(inst_f, finalizer = fin_f, config = Config::default()),
            ],
        };
        let registry_b = async_registry! {
            scope(DefaultScope::App) [
                provide(async || Ok(())),
                provide(async |Inject(_): Inject<()>| Ok(((), ()))),
                provide(inst_c, config = Config::default()),
                provide(inst_d, finalizer = fin_d),
                provide(inst_e, config = Config::default(), finalizer = fin_e),
                provide(inst_f, finalizer = fin_f, config = Config::default()),
            ],
            provide(DefaultScope::Request, inst_a),
        };

        assert_eq!(registry_a.registry.entries.len(), 7);
        assert_eq!(registry_a.sync.entries.len(), 1);
        assert_eq!(registry_b.registry.entries.len(), 7);
        assert_eq!(registry_b.sync.entries.len(), 1);
    }

    #[test]
    #[traced_test]
    fn test_entry_in_scope() {
        async_registry_internal! { @entries_in_scope scope(DefaultScope::App) [ provide(inst_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_in_scope_with_config() {
        async_registry_internal! { @entries_in_scope scope(DefaultScope::App) [ provide(inst_a, config = Config::default()) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_in_scope_with_finalizer() {
        async_registry_internal! { @entries_in_scope scope(DefaultScope::App) [ provide(inst_a, finalizer = fin_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_in_scope_with_config_and_finalizer() {
        async_registry_internal! { @entries_in_scope scope(DefaultScope::App) [ provide(inst_a, config = Config::default(), finalizer = fin_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_in_scope_with_finalizer_and_config_swapped() {
        async_registry_internal! { @entries_in_scope scope(DefaultScope::App) [ provide(inst_a, finalizer = fin_a, config = Config::default()) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_scope() {
        async_registry_internal! { @entries_with_scope provide(DefaultScope::App, inst_a) };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_scope_with_config() {
        async_registry_internal! { @entries_with_scope provide(DefaultScope::App, inst_a, config = Config::default()) };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_scope_with_finalizer() {
        async_registry_internal! { @entries_with_scope provide(DefaultScope::App, inst_a, finalizer = fin_a) };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_scope_with_config_and_finalizer() {
        async_registry_internal! { @entries_with_scope provide(DefaultScope::App, inst_a, config = Config::default(), finalizer = fin_a) };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_scope_with_finalizer_and_config_swapped() {
        async_registry_internal! { @entries_with_scope provide(DefaultScope::App, inst_a, finalizer = fin_a, config = Config::default()) };
    }

    #[test]
    #[traced_test]
    fn test_multiple_entries_in_scope() {
        async_registry_internal! {
            @entries_in_scope
            scope(DefaultScope::App) [
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
    fn test_multiple_entries_with_scope() {
        async_registry_internal! {
            @entries_with_scope
            provide(DefaultScope::App, inst_a),
            provide(DefaultScope::App, inst_b),
            provide(DefaultScope::App, inst_c, config = Config::default(), finalizer = fin_c),
            provide(DefaultScope::App, inst_d, finalizer = fin_d),
            provide(DefaultScope::App, inst_e, config = Config::default(), finalizer = fin_e),
        };
    }

    #[test]
    #[traced_test]
    fn test_entries_in_scope_trailing_comma_and_spaces() {
        async_registry_internal! {
            @entries_in_scope
            scope(DefaultScope::App) [
                provide(inst_a, config = Config::default(), finalizer = fin_a),
            ]
        };
    }

    #[test]
    #[traced_test]
    fn test_entries_with_scope_trailing_comma_and_spaces() {
        async_registry_internal! {
            @entries_with_scope
            provide(DefaultScope::App, inst_a, config = Config::default(), finalizer = fin_a),
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_entries_in_scope() {
        let registry = async_registry! {
            scope(DefaultScope::App) [
                provide(inst_a),
                provide(inst_b),
                provide(inst_c, config = Config::default()),
                provide(inst_d, finalizer = fin_d),
                provide(inst_e, config = Config::default(), finalizer = fin_e),
            ],
        };

        assert_eq!(registry.registry.entries.len(), 6);
        assert_eq!(registry.sync.entries.len(), 1);
    }

    #[test]
    #[traced_test]
    fn test_registry_entries_with_scope() {
        let registry = async_registry! {
            provide(DefaultScope::App, inst_a),
            provide(DefaultScope::App, inst_b),
            provide(DefaultScope::App, inst_c, config = Config::default()),
            provide(DefaultScope::App, inst_d, finalizer = fin_d),
            provide(DefaultScope::App, inst_e, config = Config::default(), finalizer = fin_e),
        };

        assert_eq!(registry.registry.entries.len(), 6);
        assert_eq!(registry.sync.entries.len(), 1);
    }

    #[test]
    #[traced_test]
    fn test_registry_entries_in_scope_multiple_scopes() {
        let registry = async_registry! {
            scope(DefaultScope::App) [
                provide(inst_a),
                provide(inst_b),
            ],
            scope(DefaultScope::Request) [
                provide(inst_c, config = Config::default()),
                provide(inst_d, finalizer = fin_d),
            ],
        };

        assert_eq!(registry.registry.entries.len(), 5);
        assert_eq!(registry.sync.entries.len(), 1);
    }

    #[test]
    #[traced_test]
    fn test_registry_entries_with_scope_multiple_scopes() {
        let registry = async_registry! {
            provide(DefaultScope::App, inst_a),
            provide(DefaultScope::App, inst_b),
            provide(DefaultScope::Request, inst_c, config = Config::default()),
            provide(DefaultScope::Request, inst_d, finalizer = fin_d),
        };

        assert_eq!(registry.registry.entries.len(), 5);
        assert_eq!(registry.sync.entries.len(), 1);
    }

    #[test]
    #[traced_test]
    fn test_registry_empty_scope() {
        let registry = async_registry! {};

        assert_eq!(registry.registry.entries.len(), 1);
        assert_eq!(registry.sync.entries.len(), 1);
    }

    #[test]
    #[traced_test]
    fn test_registry_trailing_commas_and_spacing() {
        let registry = async_registry! {
            scope(DefaultScope::App)[
                provide(inst_a),
                provide(inst_b , config = Config::default() , finalizer = fin_b ,)
            ]
            , scope(DefaultScope::Request)[ provide(inst_c) , ]
        };

        assert_eq!(registry.registry.entries.len(), 4);
        assert_eq!(registry.sync.entries.len(), 1);
    }

    #[test]
    #[traced_test]
    fn test_registry_get() {
        let RegistryWithSync { registry, sync } = async_registry! {
            scope(DefaultScope::Session) [provide(inst_a), provide(inst_b), provide(inst_c)],
            scope(DefaultScope::Request) [provide(inst_d), provide(inst_e), provide(inst_f)],
        };

        assert_eq!(registry.entries.len(), 7);
        assert_eq!(sync.entries.len(), 1);

        assert!(registry.get(&TypeInfo::of::<()>()).is_some());
        assert!(registry.get(&TypeInfo::of::<((), ())>()).is_some());
        assert!(registry.get(&TypeInfo::of::<((), (), ())>()).is_some());
        assert!(registry.get(&TypeInfo::of::<((), (), (), ())>()).is_some());
        assert!(registry.get(&TypeInfo::of::<((), (), (), (), ())>()).is_some());
        assert!(registry.get(&TypeInfo::of::<((), (), (), (), (), ())>()).is_some());
        assert!(registry.get(&TypeInfo::of::<((), (), (), (), (), (), ())>()).is_none());
    }

    #[test]
    #[traced_test]
    fn test_registry_dfs_detect_ok() {
        struct A;
        struct B(A);
        struct C(B, A);

        let RegistryWithSync { registry, sync } = async_registry! {
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

        assert_eq!(registry.entries.len(), 4);
        assert_eq!(sync.entries.len(), 1);
    }

    #[test]
    #[traced_test]
    fn test_registry_dfs_detect_single() {
        struct A;

        let RegistryWithSync { registry, sync } = async_registry! {
            scope(DefaultScope::App) [
                provide(async |InjectTransient(_): InjectTransient<A>| Ok(A)),
            ],
        };
        registry.dfs_detect().unwrap_err();

        assert_eq!(registry.entries.len(), 2);
        assert_eq!(sync.entries.len(), 1);
    }

    #[test]
    #[traced_test]
    fn test_registry_dfs_detect_many() {
        struct A;
        struct B;

        let RegistryWithSync { registry, sync } = async_registry! {
            scope(DefaultScope::App) [
                provide(async |InjectTransient(_): InjectTransient<B>| Ok(A)),
            ],
            scope(DefaultScope::Session) [
                provide(async |InjectTransient(_): InjectTransient<A>| Ok(B)),
            ],
        };
        registry.dfs_detect().unwrap_err();

        assert_eq!(registry.entries.len(), 3);
        assert_eq!(sync.entries.len(), 1);
    }

    #[test]
    #[traced_test]
    fn registry_extend_entries() {
        let RegistryWithSync { registry, sync } = async_registry! {
            provide(DefaultScope::App, inst_a),
            scope(DefaultScope::Session) [provide(inst_b)],
            provide(DefaultScope::App, inst_c),
            extend(
                async_registry! {
                    scope(DefaultScope::App) [provide(inst_d)],
                    extend(
                        async_registry! {
                            scope(DefaultScope::Session) [provide(inst_e)],
                        },
                    ),
                },
            ),
            extend(
                async_registry! {
                    scope(DefaultScope::Session) [provide(inst_f)],
                },
            ),
        };

        assert_eq!(registry.entries.len(), 7);
        assert_eq!(sync.entries.len(), 1);
    }

    #[test]
    #[traced_test]
    fn test_registry_with_sync() {
        let RegistryWithSync { registry, sync } = async_registry! {
            provide(DefaultScope::App, inst_a),
            provide(DefaultScope::App, inst_b),
            sync = registry! {
                scope(DefaultScope::Session) [
                    provide(|| Ok(((), ()))),
                ],
                provide(DefaultScope::App, || Ok(((), (), ()))),
            },
        };

        assert_eq!(registry.entries.len(), 3);
        assert_eq!(sync.entries.len(), 3);
    }

    #[test]
    #[traced_test]
    fn test_registry_with_sync_and_extend() {
        let registry = async_registry! {
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
            sync = registry! {},
        };

        assert_eq!(registry.registry.entries.len(), 3);
        assert_eq!(registry.sync.entries.len(), 1);
    }
}
