use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    vec::Vec,
};

use crate::{
    any::TypeInfo,
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

impl Registry {
    #[inline]
    pub(crate) fn get(&self, type_info: &TypeInfo) -> Option<&InstantiatorData> {
        self.entries.get(type_info)
    }

    #[inline]
    #[must_use]
    pub(crate) fn get_scope_with_child_scopes(&self) -> ScopeDataWithChildScopesData {
        ScopeDataWithChildScopesData::new_with_sort(self.scopes_data.clone().into_iter().collect())
    }

    pub fn dfs_detect(&self) -> Result<(), DFSErrorKind> {
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

/// The `registry!` macro is used to create a dependency registry with various configuration options.
///
/// ### `provide` syntax
///
/// Each `provide` item defines a single dependency registration.
/// The following forms are supported:
///
/// ```no_code
/// provide(inst)                             // factory only
/// provide(inst, config = Config::default()) // with configuration
/// provide(inst, finalizer = fin)            // with finalizer
/// provide(inst, config = Config::default(), finalizer = fin) // with both parameters
/// provide(inst, finalizer = fin, config = Config::default()) // order doesn’t matter
/// ```
///
/// Parameters:
/// - `config` *(optional)* — configuration object.
/// - `finalizer` *(optional)* — function called when the dependency is finalized.
///
/// ## Usage patterns
///
/// ### 1. Single `scope`
/// ```rust
/// use froodi::{registry, InstantiateErrorKind, DefaultScope::*};
///
/// fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// registry! {
///     scope(App) [
///         provide(inst),
///     ]
/// };
/// ```
///
/// ### 2. Multiple `scope`
/// ```rust
/// use froodi::{registry, InstantiateErrorKind, DefaultScope::*};
///
/// fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// registry! {
///     scope(App) [ provide(inst) ],
///     scope(Session) [ provide(inst) ],
/// };
/// ```
///
/// ### 3. Single `provide`
/// ```rust
/// use froodi::{registry, InstantiateErrorKind, DefaultScope::*};
///
/// fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// registry! {
///     provide(App, inst)
/// };
/// ```
///
/// ### 4. Multiple `provide`
/// ```rust
/// use froodi::{registry, InstantiateErrorKind, DefaultScope::*};
///
/// fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// registry! {
///     provide(App, inst),
///     provide(Session, inst),
///     provide(Request, inst),
/// };
/// ```
///
/// ### 5. Combination of one or more `scope` and `provide`
/// ```rust
/// use froodi::{registry, InstantiateErrorKind, DefaultScope::*};
///
/// fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// registry! {
///     scope(App) [ provide(inst) ],
///     provide(Session, inst),
///     provide(Request, inst),
/// };
/// ```
///
/// ### 6. Using `extend` standalone
/// ```rust
/// use froodi::registry;
///
/// registry! {
///     extend(registry!())
/// };
/// ```
///
/// ### 7. Using `extend` together with a combination of `scope` and `provide`
/// ```rust
/// use froodi::{registry, InstantiateErrorKind, DefaultScope::*};
///
/// fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// registry! {
///     scope(App) [ provide(inst) ],
///     provide(Session, inst),
///     extend(registry!(), registry!()),
/// };
/// ```
///
/// ### 8. Empty macro usage
/// ```rust
/// use froodi::registry;
///
/// let registry = registry!();
/// ```
/// In this case, a registry with default entries is created.
#[macro_export]
macro_rules! registry {
    () => {{
        $crate::Registry::new_with_default_entries()
    }};
    (scope($scope:expr $(,)?) [ $($entries:tt)+ ], $($rest:tt)+) => {{
        let registry = $crate::utils::Merge::merge(
            $crate::macros_utils::sync::build_registry(($scope, $crate::registry_internal! { scope($scope) [ $($entries)+ ] })),
            $crate::registry_internal! { $($rest)+ }
        );
        registry.dfs_detect().unwrap();
        registry
    }};
    (scope($scope:expr $(,)?) [ $($entries:tt)+ ] $(,)?) => {{
        let registry = $crate::macros_utils::sync::build_registry(($scope, $crate::registry_internal! { scope($scope) [ $($entries)+ ] }));
        registry.dfs_detect().unwrap();
        registry
    }};
    (provide($scope:expr, $($entry:tt)+), $($rest:tt)+) => {{
        let registry = $crate::utils::Merge::merge(
            $crate::macros_utils::sync::build_registry(($scope, $crate::registry_internal! { provide($scope, $($entry)+) })),
            $crate::registry_internal! { $($rest)+ }
        );
        registry.dfs_detect().unwrap();
        registry
    }};
    (provide($scope:expr, $($entry:tt)+) $(,)?) => {{
        $crate::macros_utils::sync::build_registry(($scope, $crate::registry_internal! { provide($scope, $($entry)+) }))
    }};
    (extend($registry:expr $(, $($registries:expr),+ )? $(,)?) $(,)?) => {{
        #[allow(unused_mut)]
        let mut registry: $crate::Registry = $registry;
        $(
            $(
                let registry_to_merge: $crate::Registry = $registries;
                registry = $crate::utils::Merge::merge(registry, registry_to_merge);
            )+
        )?
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

#[macro_export]
#[doc(hidden)]
macro_rules! registry_internal {
    (scope($scope:expr $(,)?) [ $($entries:tt)+ ], $($rest:tt)+) => {{
        $crate::macros_utils::aliases::hlist![
            $crate::registry_internal! { @entries_in_scope scope($scope) [ $($entries)+ ] },
            $crate::registry_internal! { $($rest)+ }
        ]
    }};
    (scope($scope:expr $(,)?) [ $($entries:tt)+ ] $(,)?) => {{
        $crate::registry_internal! { @entries_in_scope scope($scope) [ $($entries)+ ] }
    }};

    (provide($scope:expr,, $($entry:tt)*) $($rest:tt)*) => {
        compile_error!("Unexpected double comma after scope in `provide` entry")
    };

    (provide($scope:expr, $($entry:tt)+), $($rest:tt)+) => {{
        $crate::macros_utils::aliases::hlist![
            $crate::registry_internal! { @entries_with_scope provide($scope, $($entry)+) },
            $crate::registry_internal! { $($rest)+ }
        ]
    }};
    (provide($scope:expr, $($entry:tt)+) $(,)?) => {{
        $crate::registry_internal! { @entries_with_scope provide($scope, $($entry)*) }
    }};
    (extend($registry:expr $(, $($registries:expr),+ )? $(,)?) $(,)?) => {{
        #[allow(unused_mut)]
        let mut registry: $crate::Registry = $registry;
        $(
            $(
                let registry_to_merge: $crate::Registry = $registries;
                registry = $crate::utils::Merge::merge(registry, registry_to_merge);
            )+
        )?
        $crate::macros_utils::types::RegistryOrEntry::Registry(registry)
    }};

    (@entries_with_scope $( provide($scope:expr, $($entry:tt)+) ),+ $(,)?) => {{
        $crate::macros_utils::aliases::hlist![$( $crate::registry_internal! { @entry scope($scope), $($entry)+ } ),+]
    }};
    (@entries_in_scope scope($scope:expr) [ $( provide($($entry:tt)+) ),+ $(,)? ]) => {{
        $crate::macros_utils::aliases::hlist![$( $crate::registry_internal! { @entry scope($scope), $($entry)+ } ),+]
    }};
    (@entry scope($scope:expr), $inst:expr $(,)?) => {{
        $crate::macros_utils::types::RegistryOrEntry::Entry(
            $crate::macros_utils::sync::make_entry($scope, $inst, None, None::<$crate::macros_utils::sync::FinDummy<_>>)
        )
    }};
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr $(,)?) => {{
        $crate::macros_utils::types::RegistryOrEntry::Entry(
            $crate::macros_utils::sync::make_entry($scope, $inst, Some($cfg), None::<$crate::macros_utils::sync::FinDummy<_>>)
        )
    }};
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr $(,)?) => {{
        $crate::macros_utils::types::RegistryOrEntry::Entry($crate::macros_utils::sync::make_entry($scope, $inst, None, Some($fin)))
    }};
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr, finalizer = $fin:expr $(,)?) => {{
        $crate::macros_utils::types::RegistryOrEntry::Entry($crate::macros_utils::sync::make_entry($scope, $inst, Some($cfg), Some($fin)))
    }};
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr, config = $cfg:expr $(,)?) => {{
        $crate::macros_utils::types::RegistryOrEntry::Entry($crate::macros_utils::sync::make_entry($scope, $inst, Some($cfg), Some($fin)))
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

    use crate::{any::TypeInfo, utils::thread_safety::RcThreadSafety, Config, DefaultScope, Inject, InjectTransient, InstantiateErrorKind};

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
        assert_eq!(
            registry! {
                provide(DefaultScope::Request, inst_a),
                scope(DefaultScope::App) [
                    provide(|| Ok(())),
                    provide(|Inject(_): Inject<()>| Ok(((), ()))),
                    provide(inst_c, config = Config::default()),
                    provide(inst_d, finalizer = fin_d),
                    provide(inst_e, config = Config::default(), finalizer = fin_e),
                    provide(inst_f, finalizer = fin_f, config = Config::default()),
                ],
            }
            .entries
            .len(),
            7
        );
        assert_eq!(
            registry! {
                scope(DefaultScope::App) [
                    provide(|| Ok(())),
                    provide(|Inject(_): Inject<()>| Ok(((), ()))),
                    provide(inst_c, config = Config::default()),
                    provide(inst_d, finalizer = fin_d),
                    provide(inst_e, config = Config::default(), finalizer = fin_e),
                    provide(inst_f, finalizer = fin_f, config = Config::default()),
                ],
                provide(DefaultScope::Request, inst_a),
            }
            .entries
            .len(),
            7
        );
    }

    #[test]
    #[traced_test]
    fn test_entry_in_scope() {
        registry_internal! { @entries_in_scope scope(DefaultScope::App) [ provide(inst_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_in_scope_with_config() {
        registry_internal! { @entries_in_scope scope(DefaultScope::App) [ provide(inst_a, config = Config::default()) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_in_scope_with_finalizer() {
        registry_internal! { @entries_in_scope scope(DefaultScope::App) [ provide(inst_a, finalizer = fin_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_in_scope_with_config_and_finalizer() {
        registry_internal! { @entries_in_scope scope(DefaultScope::App) [ provide(inst_a, config = Config::default(), finalizer = fin_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_in_scope_with_finalizer_and_config_swapped() {
        registry_internal! { @entries_in_scope scope(DefaultScope::App) [ provide(inst_a, finalizer = fin_a, config = Config::default()) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_scope() {
        registry_internal! { @entries_with_scope provide(DefaultScope::App, inst_a) };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_scope_with_config() {
        registry_internal! { @entries_with_scope provide(DefaultScope::App, inst_a, config = Config::default()) };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_scope_with_finalizer() {
        registry_internal! { @entries_with_scope provide(DefaultScope::App, inst_a, finalizer = fin_a) };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_scope_with_config_and_finalizer() {
        registry_internal! { @entries_with_scope provide(DefaultScope::App, inst_a, config = Config::default(), finalizer = fin_a) };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_scope_with_finalizer_and_config_swapped() {
        registry_internal! { @entries_with_scope provide(DefaultScope::App, inst_a, finalizer = fin_a, config = Config::default()) };
    }

    #[test]
    #[traced_test]
    fn test_multiple_entries_in_scope() {
        registry_internal! {
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
        registry_internal! {
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
        registry_internal! {
            @entries_in_scope
            scope(DefaultScope::App) [
                provide(inst_a, config = Config::default(), finalizer = fin_a),
            ]
        };
    }

    #[test]
    #[traced_test]
    fn test_entries_with_scope_trailing_comma_and_spaces() {
        registry_internal! {
            @entries_with_scope
            provide(DefaultScope::App, inst_a, config = Config::default(), finalizer = fin_a),
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_entries_in_scope() {
        assert_eq!(
            registry! {
                scope(DefaultScope::App) [
                    provide(inst_a),
                    provide(inst_b),
                    provide(inst_c, config = Config::default()),
                    provide(inst_d, finalizer = fin_d),
                    provide(inst_e, config = Config::default(), finalizer = fin_e),
                ],
            }
            .entries
            .len(),
            6
        );
    }

    #[test]
    #[traced_test]
    fn test_registry_entries_with_scope() {
        assert_eq!(
            registry! {
                provide(DefaultScope::App, inst_a),
                provide(DefaultScope::App, inst_b),
                provide(DefaultScope::App, inst_c, config = Config::default()),
                provide(DefaultScope::App, inst_d, finalizer = fin_d),
                provide(DefaultScope::App, inst_e, config = Config::default(), finalizer = fin_e),
            }
            .entries
            .len(),
            6
        );
    }

    #[test]
    #[traced_test]
    fn test_registry_entries_in_scope_multiple_scopes() {
        assert_eq!(
            registry! {
                scope(DefaultScope::App) [
                    provide(inst_a),
                    provide(inst_b),
                ],
                scope(DefaultScope::Request) [
                    provide(inst_c, config = Config::default()),
                    provide(inst_d, finalizer = fin_d),
                ],
            }
            .entries
            .len(),
            5
        );
    }

    #[test]
    #[traced_test]
    fn test_registry_entries_with_scope_multiple_scopes() {
        assert_eq!(
            registry! {
                provide(DefaultScope::App, inst_a),
                provide(DefaultScope::App, inst_b),
                provide(DefaultScope::Request, inst_c, config = Config::default()),
                provide(DefaultScope::Request, inst_d, finalizer = fin_d),
            }
            .entries
            .len(),
            5
        );
    }

    #[test]
    #[traced_test]
    fn test_registry_empty_scope() {
        assert_eq!(registry! {}.entries.len(), 1)
    }

    #[test]
    #[traced_test]
    fn test_registry_trailing_commas_and_spacing() {
        assert_eq!(
            registry! {
                scope(DefaultScope::App)[
                    provide(inst_a),
                    provide(inst_b , config = Config::default() , finalizer = fin_b ,)
                ]
                , scope(DefaultScope::Request)[ provide(inst_c) , ]
            }
            .entries
            .len(),
            4
        )
    }

    #[test]
    #[traced_test]
    fn test_registry_get() {
        let registry = registry! {
            scope(DefaultScope::Session) [provide(inst_a), provide(inst_b), provide(inst_c)],
            scope(DefaultScope::Request) [provide(inst_d), provide(inst_e), provide(inst_f)],
        };

        assert_eq!(registry.entries.len(), 7);

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

        assert_eq!(registry.entries.len(), 4);
    }

    #[test]
    #[should_panic]
    #[traced_test]
    fn test_registry_dfs_detect_single() {
        struct A;

        registry! {
            scope(DefaultScope::App) [
                provide(|InjectTransient(_): InjectTransient<A>| Ok(A)),
            ],
        };
    }

    #[test]
    #[should_panic]
    #[traced_test]
    fn test_registry_dfs_detect_many() {
        struct A;
        struct B;

        registry! {
            scope(DefaultScope::App) [
                provide(|InjectTransient(_): InjectTransient<B>| Ok(A)),
            ],
            scope(DefaultScope::Session) [
                provide(|InjectTransient(_): InjectTransient<A>| Ok(B)),
            ],
        };
    }

    #[test]
    #[traced_test]
    fn registry_extend_entries() {
        let registry = registry! {
            provide(DefaultScope::App, inst_a),
            scope(DefaultScope::Session) [provide(inst_b)],
            provide(DefaultScope::App, inst_c),
            extend(
                registry! {
                    scope(DefaultScope::App) [provide(inst_d)],
                    extend(
                        registry! {
                            scope(DefaultScope::Session) [provide(inst_e)],
                        },
                    ),
                },
                registry! {
                    scope(DefaultScope::Session) [provide(inst_f)],
                },
            ),
        };

        assert_eq!(registry.entries.len(), 7);
    }
}
