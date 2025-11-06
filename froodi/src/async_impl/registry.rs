use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    vec::Vec,
};

use crate::{
    any::TypeInfo,
    async_impl::{
        autowired::__GLOBAL_ASYNC_ENTRY_GETTERS,
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

        for getter in __GLOBAL_ASYNC_ENTRY_GETTERS {
            let (type_info, instantiator_data) = getter();
            entries.insert(type_info, instantiator_data);
        }

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

#[derive(Clone, Default)]
pub struct RegistryWithSync {
    pub registry: Registry,
    pub sync: SyncRegistry,
}

impl RegistryWithSync {
    pub fn dfs_detect(&self) -> Result<(), DFSErrorKind> {
        self.registry.dfs_detect()?;
        self.sync.dfs_detect()
    }
}

#[allow(clippy::default_trait_access)]
impl From<Registry> for RegistryWithSync {
    fn from(registry: Registry) -> Self {
        Self {
            registry,
            sync: Default::default(),
        }
    }
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
/// ### 6.Using `extend` standalone
/// ```rust
/// use froodi::{async_registry, DefaultScope::*};
///
/// async_registry! {
///     extend(async_registry!())
/// };
/// ```
///
/// ### 7. Using `extend` with different registries
/// ```rust
/// use froodi::{registry, async_registry, DefaultScope::*};
///
/// async_registry! {
///     extend(async_registry!(), registry!()),
/// };
/// ```
///
/// ### 8. Using `extend` together with a combination of `scope` and `provide`
/// ```rust
/// use froodi::{registry, async_registry, InstantiateErrorKind, DefaultScope::*};
///
/// async fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// async_registry! {
///     scope(App) [ provide(inst) ],
///     provide(Session, inst),
///     extend(async_registry!(), registry!()),
/// };
/// ```
/// **Attention**: `extend` must be the last macro invocation.
///
/// ### 9. Empty macro usage
/// ```rust
/// use froodi::async_registry;
///
/// let registry = async_registry!();
/// ```
#[macro_export]
macro_rules! async_registry {
    () => {{
        $crate::async_impl::RegistryWithSync {
            registry: $crate::async_impl::Registry::new_with_default_entries(),
            sync: $crate::Registry::new_with_default_entries(),
        }
    }};
    (scope($scope:expr $(,)?) [ $($entries:tt)+ ], $($rest:tt)+) => {{
        let registry = $crate::utils::Merge::merge(
            $crate::macros_utils::async_impl::build_registry(($scope, $crate::async_registry_internal! { scope($scope) [ $($entries)+ ] })),
            $crate::async_registry_internal! { $($rest)+ }
        );
        registry.dfs_detect().unwrap();
        registry
    }};
    (scope($scope:expr $(,)?) [ $($entries:tt)+ ] $(,)?) => {{
        let registry = $crate::macros_utils::async_impl::build_registry(($scope, $crate::async_registry_internal! { scope($scope) [ $($entries)+ ] }));
        registry.dfs_detect().unwrap();
        registry
    }};
    (provide($scope:expr, $($entry:tt)+), $($rest:tt)+) => {{
        let registry = $crate::utils::Merge::merge(
            $crate::macros_utils::async_impl::build_registry(($scope, $crate::async_registry_internal! { provide($scope, $($entry)+) })),
            $crate::async_registry_internal! { $($rest)+ }
        );
        registry.dfs_detect().unwrap();
        registry
    }};
    (provide($scope:expr, $($entry:tt)+) $(,)?) => {{
        let registry = $crate::macros_utils::async_impl::build_registry(($scope, $crate::async_registry_internal! { provide($scope, $($entry)+) }));
        registry.dfs_detect().unwrap();
        registry
    }};
    (extend($($registries:expr),+ $(,)?) $(,)?) => {{
        let mut registry = $crate::async_impl::RegistryWithSync {
            registry: $crate::async_impl::Registry::new_with_default_entries(),
            sync: $crate::Registry::new_with_default_entries(),
        };
        $(
            registry = $crate::utils::Merge::merge(registry, $registries);
        )+
        registry.dfs_detect().unwrap();
        registry
    }};

    (scope() $($rest:tt)*) => {
        compile_error!("`scope` block must have a scope")
    };
    (scope($scope:expr) [] $($rest:tt)*) => {
        compile_error!("`scope` block must contain at least one entry")
    };
    (scope($scope:expr $(,)?) [ $($entries:tt)* ] $($rest:tt)+) => {
        compile_error!("Missing comma after `scope` block")
    };
    (provide() $($rest:tt)*) => {
        compile_error!("`provide` must have a scope and an instantiator")
    };
    (provide($scope:expr $(,)?) $($rest:tt)*) => {
        compile_error!("`provide` must include an instantiator after the scope")
    };
    (provide(, $($entity:tt)+) $($rest:tt)*) => {
        compile_error!("`provide` must include a scope before the instantiator")
    };
    (provide($($entry:tt)*) $($rest:tt)+) => {
        compile_error!("Missing comma after `provide` block")
    };
    (extend() $($rest:tt)*) => {
        compile_error!("`extend` macro must be called with at least one argument")
    };
    (extend($($entry:tt)*) $($rest:tt)+) => {
        compile_error!("Missing comma after/in `extend` block or unexpected comma in the block")
    };
    (,) => {
        compile_error!("Duplicate or unexpected comma")
    };
    ($($rest:tt)*) => {
        compile_error!(concat!("Unknown syntax: ", stringify!($($rest)*)))
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! async_registry_internal {
    (scope($scope:expr $(,)?) [ $($entries:tt)+ ], $($rest:tt)+) => {{
        $crate::macros_utils::aliases::hlist![
            $crate::async_registry_internal! { @entries_in_scope scope($scope) [ $($entries)+ ] },
            $crate::async_registry_internal! { $($rest)+ }
        ]
    }};
    (scope($scope:expr $(,)?) [ $($entries:tt)+ ] $(,)?) => {{
        $crate::async_registry_internal! { @entries_in_scope scope($scope) [ $($entries)+ ] }
    }};

    (provide($scope:expr,, $($entry:tt)*) $($rest:tt)*) => {
        compile_error!("Unexpected double comma after scope in `provide` entry")
    };

    (provide($scope:expr, $($entry:tt)+), $($rest:tt)+) => {{
        $crate::macros_utils::aliases::hlist![
            $crate::async_registry_internal! { @entries_with_scope provide($scope, $($entry)+) },
            $crate::async_registry_internal! { $($rest)+ }
        ]
    }};
    (provide($scope:expr, $($entry:tt)+) $(,)?) => {{
        $crate::async_registry_internal! { @entries_with_scope provide($scope, $($entry)*) }
    }};
    (extend($($registries:expr),+ $(,)?) $(,)?) => {{
        let mut registry_kind = $crate::macros_utils::types::RegistryKind::AsyncWithSync(Default::default());
        $(
            registry_kind = $crate::utils::Merge::merge(registry_kind, $registries);
        )+
        $crate::macros_utils::types::RegistryKindOrEntry::Kind(registry_kind)
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

    (@entries_in_scope scope($scope:expr) [ $($entry:tt)+ ]) => {
        compile_error!("`scope` block supports only non empty `provide` entries")
    };
    (@entry scope($scope:expr), $inst:expr,, $($rest:tt)*) => {
        compile_error!("Unexpected double comma in `provide` entry")
    };
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr,, $($rest:tt)*) => {
        compile_error!("Unexpected double comma after `config` in `provide` entry")
    };
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr,, $($rest:tt)*) => {
        compile_error!("Unexpected double comma after `finalizer` in `provide` entry")
    };
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr, finalizer = $fin:expr,, $($rest:tt)*) => {
        compile_error!("Unexpected double comma after entry arguments")
    };
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr, config = $cfg:expr,, $($rest:tt)*) => {
        compile_error!("Unexpected double comma after entry arguments")
    };
    (@entry scope($scope:expr), $inst:expr, $($rest:tt)*) => {
        compile_error!(concat!("One of parameter in `provide` entry is unexpected: ", stringify!($($rest)*)))
    };

    (scope() $($rest:tt)*) => {
        compile_error!("`scope` block must have a scope")
    };
    (scope($scope:expr) [] $($rest:tt)*) => {
        compile_error!("`scope` block must contain at least one entry")
    };
    (scope($scope:expr $(,)?) [ $($entries:tt)* ] $($rest:tt)+) => {
        compile_error!("Missing comma after `scope` block")
    };
    (provide() $($rest:tt)*) => {
        compile_error!("`provide` must have a scope and an instantiator")
    };
    (provide($scope:expr $(,)?) $($rest:tt)*) => {
        compile_error!("`provide` must include an instantiator after the scope")
    };
    (provide(, $($entity:tt)+) $($rest:tt)*) => {
        compile_error!("`provide` must include a scope before the instantiator")
    };
    (provide($($entry:tt)*) $($rest:tt)+) => {
        compile_error!("Missing comma after `provide` block")
    };
    (extend($($entry:tt)*), $($rest:tt)+) => {
        compile_error!("`extend` macro must be at the last macro invocation")
    };
    (extend() $($rest:tt)*) => {
        compile_error!("`extend` macro must be called with at least one argument")
    };
    (extend($($entry:tt)*) $($rest:tt)+) => {
        compile_error!("Missing comma after/in `extend` block or unexpected comma in the block")
    };
    (,) => {
        compile_error!("Duplicate or unexpected comma")
    };
    ($($rest:tt)*) => {
        compile_error!(concat!("Unknown syntax: ", stringify!($($rest)*)))
    };
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
    #[should_panic]
    #[traced_test]
    fn test_registry_dfs_detect_single() {
        struct A;

        async_registry! {
            scope(DefaultScope::App) [
                provide(async |InjectTransient(_): InjectTransient<A>| Ok(A)),
            ],
        };
    }

    #[test]
    #[should_panic]
    #[traced_test]
    fn test_registry_dfs_detect_many() {
        struct A;
        struct B;

        async_registry! {
            scope(DefaultScope::App) [
                provide(async |InjectTransient(_): InjectTransient<B>| Ok(A)),
            ],
            scope(DefaultScope::Session) [
                provide(async |InjectTransient(_): InjectTransient<A>| Ok(B)),
            ],
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_extend_entries() {
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
                            extend(registry! {
                                provide(DefaultScope::App, || Ok(((), (), ()))),
                                extend(
                                    registry! {
                                        extend(registry! {})
                                    },
                                    registry! {},
                                    registry! {},
                                ),
                            }),
                        },
                    ),
                },
                async_registry! {
                    scope(DefaultScope::Session) [provide(inst_f)],
                },
                registry! {
                    provide(DefaultScope::Session, || Ok(((), ()))),
                    extend(registry! {}, registry! {}, registry! {}),
                },
            ),
        };

        assert_eq!(registry.entries.len(), 7);
        assert_eq!(sync.entries.len(), 3);
    }
}
