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

#[derive(Clone, Default)]
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
            #[allow(clippy::similar_names)]
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
    #[must_use]
    pub fn new_with_default_entries() -> Self {
        Self::new::<DefaultScope, DefaultScope, 6>(BTreeMap::new())
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
/// ### 7. Using multiple `extend`
/// ```rust
/// use froodi::registry;
///
/// registry! {
///     extend(registry!(), registry!()),
///     extend(registry!()),
/// };
/// ```
///
/// ### 8. Using `extend` together with a combination of `scope` and `provide`
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
/// ### 9. Empty macro usage
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
    (scope($scope:expr) [ $($entries:tt)+ ], $($rest:tt)+) => {{
        $crate::utils::Merge::merge(
            $crate::macros_utils::sync::build_registry(($scope, $crate::registry_internal! { scope($scope) [ $($entries)+ ] })),
            $crate::registry_internal! { $($rest)+ }
        )
    }};
    (scope($scope:expr) [ $($entries:tt)+ ] $(,)?) => {{
        $crate::macros_utils::sync::build_registry(($scope, $crate::registry_internal! { scope($scope) [ $($entries)+ ] }))
    }};
    (provide($scope:expr, $($entry:tt)+), $($rest:tt)+) => {{
        $crate::utils::Merge::merge(
            $crate::macros_utils::sync::build_registry(($scope, $crate::registry_internal! { provide($scope, $($entry)+) })),
            $crate::registry_internal! { $($rest)+ }
        )
    }};
    (provide($scope:expr, $($entry:tt)+ ) $(,)?) => {{
        $crate::macros_utils::sync::build_registry(($scope, $crate::registry_internal! { provide($scope, $($entry)+) }))
    }};
    ($( extend( $($registries:expr),+ $(,)? ) ),+ $(,)?) => {{
        let mut registry = $crate::Registry::new_with_default_entries();
        $(
            $(
                registry = $crate::utils::Merge::merge(registry, $registries);
            )+
        )+
        registry
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! registry_internal {
    (scope($scope:expr) [ $($entries:tt)+ ], $($rest:tt)+) => {{
        $crate::macros_utils::aliases::hlist![
            $crate::registry_internal! { @entries_in_scope scope($scope) [ $($entries)+ ] },
            $crate::registry_internal! { $($rest)+ }
        ]
    }};
    (scope($scope:expr) [ $($entries:tt)+ ] $(,)?) => {{
        $crate::registry_internal! { @entries_in_scope scope($scope) [ $($entries)+ ] }
    }};
    (provide($scope:expr, $($entry:tt)+), $($rest:tt)+) => {{
        $crate::macros_utils::aliases::hlist![
            $crate::registry_internal! { @entries_with_scope provide($scope, $($entry)+) },
            $crate::registry_internal! { $($rest)+ }
        ]
    }};
    (provide($scope:expr, $($entry:tt)+) $(,)?) => {{
        $crate::registry_internal! { @entries_with_scope provide($scope, $($entry)*) }
    }};
    ($( extend( $($registries:expr),+ $(,)? ) ),+ $(,)?) => {{
        let mut registry = $crate::Registry::default();
        $(
            $(
                registry = $crate::utils::Merge::merge(registry, $registries);
            )+
        )+
        registry
    }};

    (@entries_with_scope $( provide($scope:expr, $($entry:tt)+) ),+ $(,)?) => {{
        $crate::macros_utils::aliases::hlist![$( $crate::registry_internal! { @entry scope($scope), $($entry)+ } ),+]
    }};
    (@entries_in_scope scope($scope:expr) [ $( provide($($entry:tt)+) ),+ $(,)? ]) => {{
        $crate::macros_utils::aliases::hlist![$( $crate::registry_internal! { @entry scope($scope), $($entry)+ } ),+]
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
            provide(DefaultScope::Request, inst_a),
            scope(DefaultScope::App) [
                provide(|| Ok(())),
                provide(|Inject(_): Inject<()>| Ok(((), ()))),
                provide(inst_c, config = Config::default()),
                provide(inst_d, finalizer = fin_d),
                provide(inst_e, config = Config::default(), finalizer = fin_e),
                provide(inst_f, finalizer = fin_f, config = Config::default()),
            ],
        };
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
        };
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
    fn test_registry_entries_with_scope() {
        registry! {
            provide(DefaultScope::App, inst_a),
            provide(DefaultScope::App, inst_b),
            provide(DefaultScope::App, inst_c, config = Config::default()),
            provide(DefaultScope::App, inst_d, finalizer = fin_d),
            provide(DefaultScope::App, inst_e, config = Config::default(), finalizer = fin_e),
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_entries_in_scope_multiple_scopes() {
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
    fn test_registry_entries_with_scope_multiple_scopes() {
        registry! {
            provide(DefaultScope::App, inst_a),
            provide(DefaultScope::App, inst_b),
            provide(DefaultScope::Request, inst_c, config = Config::default()),
            provide(DefaultScope::Request, inst_d, finalizer = fin_d),
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_empty_scope() {
        registry! {};
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
