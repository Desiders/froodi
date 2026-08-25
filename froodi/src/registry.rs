use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    vec::Vec,
};

use crate::{
    any::TypeInfo,
    dependency::{Dependency, EMPTY_DEPENDENCIES},
    errors::ValidationErrorKind,
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
        let (scope, child_scopes) = T::all();

        let mut scopes_data = Vec::with_capacity(N + 1);
        let scope_data = scope.into();

        entries.insert(
            TypeInfo::new::<Container>("Container"),
            InstantiatorData {
                instantiator: boxed_container_instantiator(),
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

        Self { entries, scopes_data }
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

    fn detect_cyclic_dependencies(&self) -> Result<(), ValidationErrorKind> {
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

    fn detect_unreachable_scopes(&self) -> Result<(), ValidationErrorKind> {
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
                    if dependency_scope.priority > scope_data.priority {
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
    // registry! {}
    () => {{
        $crate::Registry::new_with_default_entries()
    }};

    // registry! { scope() [ provide(a) ] }
    (scope() $($rest:tt)*) => {
        compile_error!("`scope` block must have a scope")
    };

    // registry! { scope(App) [] }
    (scope($scope:expr) [] $($rest:tt)*) => {
        compile_error!("`scope` block must contain at least one entry")
    };

    // registry! { scope(App, Session) [ provide(a) ] }
    (scope($scope:expr, $($more:tt)+) $($rest:tt)*) => {
        compile_error!("`scope(...)` accepts exactly one scope")
    };

    // registry! { provide() }
    (provide() $($rest:tt)*) => {
        compile_error!("`provide` must have a scope and an instantiator")
    };

    // registry! { provide(App) }
    (provide($scope:expr $(,)?) $($rest:tt)*) => {
        compile_error!("`provide` must include an instantiator after the scope")
    };

    // registry! { provide(, a) }
    (provide(, $($entity:tt)+) $($rest:tt)*) => {
        compile_error!("`provide` must include a scope before the instantiator")
    };

    // registry! { extend(r), provide(App, a) }
    (extend($($entry:tt)*), $($rest:tt)+) => {
        compile_error!("`extend` macro must be at the last macro invocation")
    };

    // registry! { extend() }
    (extend() $($rest:tt)*) => {
        compile_error!("`extend` macro must be called with at least one argument")
    };

    // registry! { extend(r) }
    // registry! { extend(r1, r2) }
    (extend($registry:expr $(, $($registries:expr),+ )? $(,)?) $(,)?) => {{
        #[allow(unused_mut)]
        let mut registry: $crate::Registry = $registry;
        $(
            $(
                let registry_to_merge: $crate::Registry = $registries;
                registry = $crate::utils::Merge::merge(registry, registry_to_merge);
            )+
        )?
        registry.validate().unwrap();
        registry
    }};

    // registry! { scope(App) [ provide(a) ], provide(Session, b), extend(r) }
    ( $( $kind:ident ( $($args:tt)* ) $([ $($body:tt)* ])? ),+ $(,)? ) => {{
        let mut registry_builder = $crate::macros_utils::sync::RegistryBuilder::new();
        // registry_internal! { @check_extend_last [ {scope} {provide} {extend} ] {scope} {provide} {extend} [] }
        $crate::registry_internal! { @check_extend_last [ $( { $kind } )+ ] $( { $kind } )+ [] }
        $(
            // registry_internal! { @clause registry_builder; scope (App) [ provide(a) ] }
            $crate::registry_internal! {
                @clause registry_builder; $kind ( $($args)* ) $( [ $($body)* ] )?
            }
        )+
        let registry = registry_builder.build();
        registry.validate().unwrap();
        registry
    }};

    // registry! { scope(App) [ provide(a) ], scope(Session) ( provide(b) ) }
    (scope($scope:expr $(,)?) [ $($entries:tt)* ], $($rest:tt)+) => {
        // registry! { scope(Session) ( provide(b) ) }
        $crate::registry! { $($rest)+ }
    };

    // registry! { provide(App, a),, provide(Session, b) }
    (provide($($entry:tt)*), $($rest:tt)+) => {
        // registry! { , provide(Session, b) }
        $crate::registry! { $($rest)+ }
    };

    // registry! { scope(App) [ provide(a) ] provide(Session, b) }
    (scope($scope:expr $(,)?) [ $($entries:tt)* ] $($rest:tt)+) => {
        compile_error!("Missing comma after `scope` block")
    };

    // registry! { scope(App) ( provide(a) ) }
    (scope($scope:expr $(,)?) ( $($entries:tt)* ) $($rest:tt)*) => {
        compile_error!("`scope(...)` entries must be wrapped in square brackets `[ ... ]`, found `( ... )`")
    };

    // registry! { scope(App) { provide(a) } }
    (scope($scope:expr $(,)?) { $($entries:tt)* } $($rest:tt)*) => {
        compile_error!("`scope(...)` entries must be wrapped in square brackets `[ ... ]`, found `{ ... }`")
    };

    // registry! { provide(App, a) provide(Session, b) }
    (provide($($entry:tt)*) $($rest:tt)+) => {
        compile_error!("Missing comma after `provide` block")
    };

    // registry! { extend(r) provide(App, a) }
    (extend($($entry:tt)*) $($rest:tt)+) => {
        compile_error!("Missing comma after/in `extend` block or unexpected comma in the block")
    };

    // registry! { , provide(App, a) }
    (, $($rest:tt)+) => {
        compile_error!("Unexpected leading or double comma")
    };

    // registry! { , }
    (,) => {
        compile_error!("Duplicate or unexpected comma")
    };

    // registry! { totally bogus }
    ($($rest:tt)*) => {
        compile_error!(concat!("Unknown syntax: ", stringify!($($rest)*)))
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! registry_internal {
    // registry! { provide(App, a), extend(r) }
    (@check_extend_last [ $($kind:tt)* ] $first_marker:tt $($marker:tt)*) => {
        // registry_internal! { @check_kind { extend } [] }
        $( $crate::registry_internal! { @check_kind $kind $marker } )*
    };

    // registry! { provide(App, a), extend(r) }
    (@check_kind { extend } []) => {};

    // registry! { provide(App, a), extend(r), provide(Session, b) }
    (@check_kind { extend } $marker:tt) => {
        compile_error!("`extend` macro must be at the last macro invocation");
    };

    // registry! { provide(App, a), provide(Session, b) }
    (@check_kind $kind:tt $marker:tt) => {};

    // registry! { provide(App, a), extend(r1, r2) }
    (@clause $builder:ident; extend ( $($registries:expr),+ $(,)? )) => {
        $(
            $builder.push_registry($registries);
        )+
    };

    // registry! { provide(App, a), extend() }
    (@clause $builder:ident; extend ()) => {
        compile_error!("`extend` macro must be called with at least one argument");
    };

    // registry! { extend(r) [ x ] }
    (@clause $builder:ident; extend ( $($registries:tt)* ) [ $($body:tt)* ]) => {
        compile_error!("Missing comma after/in `extend` block or unexpected comma in the block");
    };

    // registry! { provide(App, a), scope() [ provide(b) ] }
    (@clause $builder:ident; scope () $([ $($body:tt)* ])?) => {
        compile_error!("`scope` block must have a scope");
    };

    // registry! { provide(App, a), scope(Session) [] }
    (@clause $builder:ident; scope ( $scope:expr $(,)? ) []) => {
        compile_error!("`scope` block must contain at least one entry");
    };

    // registry! { provide(App, a), scope(Session, Request) [ provide(b) ] }
    (@clause $builder:ident; scope ( $scope:expr, $($more:tt)+ ) $([ $($body:tt)* ])?) => {
        compile_error!("`scope(...)` accepts exactly one scope");
    };

    // registry! { scope(App) [ provide(a), provide(b) ] }
    (@clause $builder:ident; scope ( $scope:expr $(,)? ) [ $( provide($($entry:tt)+) ),+ $(,)? ]) => {
        $builder.set_scope($scope);
        // registry_internal! { @entry scope(App), a }
        $( $builder.push($crate::registry_internal! { @entry scope($scope), $($entry)+ }); )+
    };

    // registry! { scope(App) [ provide(a) provide(b) ] }
    (@clause $builder:ident; scope ( $scope:expr $(,)? ) [ $($entries:tt)+ ]) => {
        compile_error!(concat!(
            "Malformed entries in `scope(...)` block: `", stringify!($($entries)+),
            "`. Entries must be a comma-separated list of `provide(...)` items (e.g. \
             `scope(App) [ provide(a), provide(b) ]`); check for a missing or extra comma, a wrong \
             separator, or an empty `provide()`."
        ));
    };

    // registry! { provide(App, a), provide() }
    (@clause $builder:ident; provide () $([ $($body:tt)* ])?) => {
        compile_error!("`provide` must have a scope and an instantiator");
    };

    // registry! { provide(App, a), provide(Session) }
    (@clause $builder:ident; provide ( $scope:expr $(,)? ) $([ $($body:tt)* ])?) => {
        compile_error!("`provide` must include an instantiator after the scope");
    };

    // registry! { provide(App, a), provide(, b) }
    (@clause $builder:ident; provide (, $($entry:tt)+ ) $([ $($body:tt)* ])?) => {
        compile_error!("`provide` must include a scope before the instantiator");
    };

    // registry! { provide(App,, a) }
    (@clause $builder:ident; provide ( $scope:expr,, $($entry:tt)* ) $([ $($body:tt)* ])?) => {
        compile_error!("Unexpected double comma after scope in `provide` entry");
    };

    // registry! { provide(App, a) }
    (@clause $builder:ident; provide ( $scope:expr, $($entry:tt)+ )) => {
        $builder.set_scope($scope);
        // registry_internal! { @entry scope(App), a }
        $builder.push($crate::registry_internal! { @entry scope($scope), $($entry)+ });
    };

    // registry! { provide(App, a) [ x ] }
    (@clause $builder:ident; provide ( $($entry:tt)* ) [ $($body:tt)* ]) => {
        compile_error!("Missing comma after `provide` block");
    };

    // registry! { scope(App) }
    (@clause $builder:ident; scope ( $($scope:tt)* )) => {
        compile_error!("`scope(...)` must be followed by its entries in square brackets `[ ... ]`");
    };

    // registry! { frobnicate(x) }
    (@clause $builder:ident; $($clause:tt)*) => {
        compile_error!(concat!("Unknown syntax: ", stringify!($($clause)*)));
    };

    // registry! { scope(App) [ provide(a) ] }
    (@entry scope($scope:expr), $inst:expr $(,)?) => {{
        $crate::macros_utils::types::RegistryOrEntry::Entry(
            $crate::macros_utils::sync::make_entry($scope, $inst, None, None::<$crate::macros_utils::sync::FinDummy<_>>)
        )
    }};

    // registry! { scope(App) [ provide(a, config = c) ] }
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr $(,)?) => {{
        $crate::macros_utils::types::RegistryOrEntry::Entry(
            $crate::macros_utils::sync::make_entry($scope, $inst, Some($cfg), None::<$crate::macros_utils::sync::FinDummy<_>>)
        )
    }};

    // registry! { scope(App) [ provide(a, finalizer = f) ] }
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr $(,)?) => {{
        $crate::macros_utils::types::RegistryOrEntry::Entry($crate::macros_utils::sync::make_entry($scope, $inst, None, Some($fin)))
    }};

    // registry! { scope(App) [ provide(a, config = c, finalizer = f) ] }
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr, finalizer = $fin:expr $(,)?) => {{
        $crate::macros_utils::types::RegistryOrEntry::Entry($crate::macros_utils::sync::make_entry($scope, $inst, Some($cfg), Some($fin)))
    }};

    // registry! { scope(App) [ provide(a, finalizer = f, config = c) ] }
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr, config = $cfg:expr $(,)?) => {{
        $crate::macros_utils::types::RegistryOrEntry::Entry($crate::macros_utils::sync::make_entry($scope, $inst, Some($cfg), Some($fin)))
    }};

    // registry! { scope(App) [ provide(a,, config = c) ] }
    (@entry scope($scope:expr), $inst:expr,, $($rest:tt)*) => {
        compile_error!("Unexpected double comma in `provide` entry")
    };

    // registry! { scope(App) [ provide(a, config = c,, finalizer = f) ] }
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr,, $($rest:tt)*) => {
        compile_error!("Unexpected double comma after `config` in `provide` entry")
    };

    // registry! { scope(App) [ provide(a, finalizer = f,, config = c) ] }
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr,, $($rest:tt)*) => {
        compile_error!("Unexpected double comma after `finalizer` in `provide` entry")
    };

    // registry! { scope(App) [ provide(a, config = c, finalizer = f,, x) ] }
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr, finalizer = $fin:expr,, $($rest:tt)*) => {
        compile_error!("Unexpected double comma after entry arguments")
    };

    // registry! { scope(App) [ provide(a, finalizer = f, config = c,, x) ] }
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr, config = $cfg:expr,, $($rest:tt)*) => {
        compile_error!("Unexpected double comma after entry arguments")
    };

    // registry! { scope(App) [ provide(a, garbage) ] }
    (@entry scope($scope:expr), $inst:expr, $($rest:tt)*) => {
        compile_error!(concat!(
            "Unexpected tokens after the instantiator in a `provide` entry: `", stringify!($($rest)*),
            "`. Expected `provide(instantiator [, config = ...] [, finalizer = ...])`; inside a \
             `scope(...)` block do not pass a scope to `provide`."
        ))
    };

    // registry! { totally bogus }
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

    /// A `Scope` implementation other than `DefaultScope`, so that the scope type and the `N` of
    /// `Scopes<N>` the `registry!` builder infers are actually exercised with a second instantiation.
    mod custom_scope {
        extern crate std;

        use crate::{
            scope::{Scope, ScopeData, Scopes},
            Container, InstantiateErrorKind,
        };
        use alloc::{
            format,
            string::{String, ToString as _},
            vec::Vec,
        };
        use tracing_test::traced_test;

        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
        enum TestScope {
            Boot,
            Work,
            Task,
        }

        impl From<TestScope> for ScopeData {
            fn from(scope: TestScope) -> Self {
                Self {
                    priority: scope.priority(),
                    name: scope.name(),
                    is_skipped_by_default: scope.is_skipped_by_default(),
                }
            }
        }

        impl Scope for TestScope {
            fn name(&self) -> &'static str {
                match self {
                    TestScope::Boot => "boot",
                    TestScope::Work => "work",
                    TestScope::Task => "task",
                }
            }

            fn priority(&self) -> u8 {
                *self as u8
            }

            fn is_skipped_by_default(&self) -> bool {
                matches!(self, TestScope::Boot)
            }
        }

        // Deliberately not 5: proves `N` is inferred from this impl and not from `DefaultScope`.
        impl Scopes<2> for TestScope {
            type Scope = Self;

            fn all() -> (Self, [Self; 2]) {
                (TestScope::Boot, [TestScope::Work, TestScope::Task])
            }
        }

        struct A;
        struct B;

        fn inst_a() -> Result<A, InstantiateErrorKind> {
            Ok(A)
        }
        fn inst_b() -> Result<B, InstantiateErrorKind> {
            Ok(B)
        }

        #[test]
        #[traced_test]
        fn test_custom_scope_fills_scopes_data_from_its_own_scopes() {
            let registry = registry! {
                scope(TestScope::Work) [ provide(inst_a) ],
            };

            assert_eq!(registry.scopes_data.len(), 3);
            assert_eq!(
                registry.scopes_data.iter().map(|s| s.name).collect::<Vec<_>>(),
                ["boot", "work", "task"]
            );
            assert_eq!(registry.scopes_data[0], TestScope::Boot.into());
        }

        #[test]
        #[traced_test]
        fn test_custom_scope_entries_keep_their_own_scope() {
            let registry = registry! {
                scope(TestScope::Work) [ provide(inst_a) ],
                provide(TestScope::Task, inst_b),
            };

            assert_eq!(registry.entries.len(), 3);
            assert_eq!(
                registry.get(&crate::any::TypeInfo::of::<A>()).unwrap().scope_data,
                TestScope::Work.into()
            );
            assert_eq!(
                registry.get(&crate::any::TypeInfo::of::<B>()).unwrap().scope_data,
                TestScope::Task.into()
            );
        }

        #[test]
        #[traced_test]
        fn test_custom_scope_container_resolves_at_start_scope() {
            let container = Container::new(registry! {
                scope(TestScope::Work) [ provide(inst_a) ],
            });

            assert!(container.get::<A>().is_ok());
        }

        #[test]
        #[traced_test]
        fn test_custom_scope_mixed_clause_forms() {
            let registry = registry! {
                scope(TestScope::Work) [ provide(inst_a), provide(inst_b) ],
                provide(TestScope::Task, || Ok(())),
            };

            assert_eq!(registry.entries.len(), 4);
        }

        #[test]
        #[should_panic]
        #[traced_test]
        fn test_custom_scope_validate_rejects_narrower_dependency() {
            registry! {
                scope(TestScope::Work) [ provide(|crate::InjectTransient(_): crate::InjectTransient<B>| Ok(A)) ],
                scope(TestScope::Task) [ provide(inst_b) ],
            };
        }
    }

    fn inst_a() -> Result<(), InstantiateErrorKind> {
        Ok(())
    }
    fn inst_b() -> Result<((), ()), InstantiateErrorKind> {
        Ok(((), ()))
    }
    fn inst_b_with_c(_dependency: InjectTransient<((), (), ())>) -> Result<((), ()), InstantiateErrorKind> {
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
                provide(DefaultScope::Runtime, inst_a),
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
                provide(DefaultScope::Runtime, inst_a),
            }
            .entries
            .len(),
            7
        );
    }

    #[test]
    #[traced_test]
    fn test_entry_in_scope() {
        let mut builder = crate::macros_utils::sync::RegistryBuilder::new();
        registry_internal! { @clause builder; scope(DefaultScope::App) [ provide(inst_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_in_scope_with_config() {
        let mut builder = crate::macros_utils::sync::RegistryBuilder::new();
        registry_internal! { @clause builder; scope(DefaultScope::App) [ provide(inst_a, config = Config::default()) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_in_scope_with_finalizer() {
        let mut builder = crate::macros_utils::sync::RegistryBuilder::new();
        registry_internal! { @clause builder; scope(DefaultScope::App) [ provide(inst_a, finalizer = fin_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_in_scope_with_config_and_finalizer() {
        let mut builder = crate::macros_utils::sync::RegistryBuilder::new();
        registry_internal! { @clause builder; scope(DefaultScope::App) [ provide(inst_a, config = Config::default(), finalizer = fin_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_in_scope_with_finalizer_and_config_swapped() {
        let mut builder = crate::macros_utils::sync::RegistryBuilder::new();
        registry_internal! { @clause builder; scope(DefaultScope::App) [ provide(inst_a, finalizer = fin_a, config = Config::default()) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_scope() {
        let mut builder = crate::macros_utils::sync::RegistryBuilder::new();
        registry_internal! { @clause builder; provide(DefaultScope::App, inst_a) };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_scope_with_config() {
        let mut builder = crate::macros_utils::sync::RegistryBuilder::new();
        registry_internal! { @clause builder; provide(DefaultScope::App, inst_a, config = Config::default()) };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_scope_with_finalizer() {
        let mut builder = crate::macros_utils::sync::RegistryBuilder::new();
        registry_internal! { @clause builder; provide(DefaultScope::App, inst_a, finalizer = fin_a) };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_scope_with_config_and_finalizer() {
        let mut builder = crate::macros_utils::sync::RegistryBuilder::new();
        registry_internal! { @clause builder; provide(DefaultScope::App, inst_a, config = Config::default(), finalizer = fin_a) };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_scope_with_finalizer_and_config_swapped() {
        let mut builder = crate::macros_utils::sync::RegistryBuilder::new();
        registry_internal! { @clause builder; provide(DefaultScope::App, inst_a, finalizer = fin_a, config = Config::default()) };
    }

    #[test]
    #[traced_test]
    fn test_multiple_entries_in_scope() {
        let mut builder = crate::macros_utils::sync::RegistryBuilder::new();
        registry_internal! {
            @clause builder;
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
        let mut builder = crate::macros_utils::sync::RegistryBuilder::new();
        registry_internal! { @clause builder; provide(DefaultScope::App, inst_a) };
        registry_internal! { @clause builder; provide(DefaultScope::App, inst_b) };
        registry_internal! { @clause builder; provide(DefaultScope::App, inst_c, config = Config::default(), finalizer = fin_c) };
        registry_internal! { @clause builder; provide(DefaultScope::App, inst_d, finalizer = fin_d) };
        registry_internal! { @clause builder; provide(DefaultScope::App, inst_e, config = Config::default(), finalizer = fin_e) };
    }

    #[test]
    #[traced_test]
    fn test_entries_in_scope_trailing_comma_and_spaces() {
        let mut builder = crate::macros_utils::sync::RegistryBuilder::new();
        registry_internal! {
            @clause builder;
            scope(DefaultScope::App) [
                provide(inst_a, config = Config::default(), finalizer = fin_a),
            ]
        };
    }

    #[test]
    #[traced_test]
    fn test_entries_with_scope_trailing_comma_and_spaces() {
        let mut builder = crate::macros_utils::sync::RegistryBuilder::new();
        registry_internal! { @clause builder; provide(DefaultScope::App, inst_a, config = Config::default(), finalizer = fin_a) };
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
        registry.validate().unwrap();

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
    #[should_panic]
    #[traced_test]
    fn test_registry_rejects_narrower_scope_dependency() {
        struct WideThing;
        struct NarrowThing;

        registry! {
            scope(DefaultScope::App) [
                provide(|InjectTransient(_): InjectTransient<NarrowThing>| Ok(WideThing)),
            ],
            scope(DefaultScope::Request) [
                provide(|| Ok(NarrowThing)),
            ],
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_extend_entries() {
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

    #[test]
    #[traced_test]
    fn test_registry_extend_keeps_root_and_extended_different_entries() {
        let registry = registry! {
            provide(DefaultScope::App, inst_a),
            extend(
                registry! {
                    provide(DefaultScope::Request, inst_b),
                },
            ),
        };

        let entry_a = registry.get(&TypeInfo::of::<()>()).unwrap();
        let entry_b = registry.get(&TypeInfo::of::<((), ())>()).unwrap();

        assert_eq!(registry.entries.len(), 3);
        assert_eq!(entry_a.scope_data, DefaultScope::App.into());
        assert_eq!(entry_b.scope_data, DefaultScope::Request.into());
    }

    #[test]
    #[traced_test]
    fn test_registry_extend_keeps_different_entries_from_multiple_registries() {
        let registry = registry! {
            extend(
                registry! {
                    provide(DefaultScope::App, inst_a),
                },
                registry! {
                    provide(DefaultScope::Request, inst_b),
                },
            ),
        };

        let entry_a = registry.get(&TypeInfo::of::<()>()).unwrap();
        let entry_b = registry.get(&TypeInfo::of::<((), ())>()).unwrap();

        assert_eq!(registry.entries.len(), 3);
        assert_eq!(entry_a.scope_data, DefaultScope::App.into());
        assert_eq!(entry_b.scope_data, DefaultScope::Request.into());
    }

    #[test]
    #[traced_test]
    fn test_registry_extend_keeps_entries_from_nested_extend() {
        let registry = registry! {
            provide(DefaultScope::App, inst_a),
            extend(
                registry! {
                    provide(DefaultScope::Session, inst_b),
                    extend(
                        registry! {
                            provide(DefaultScope::Request, inst_c),
                        },
                    ),
                },
            ),
        };

        let entry_a = registry.get(&TypeInfo::of::<()>()).unwrap();
        let entry_b = registry.get(&TypeInfo::of::<((), ())>()).unwrap();
        let entry_c = registry.get(&TypeInfo::of::<((), (), ())>()).unwrap();

        assert_eq!(registry.entries.len(), 4);
        assert_eq!(entry_a.scope_data, DefaultScope::App.into());
        assert_eq!(entry_b.scope_data, DefaultScope::Session.into());
        assert_eq!(entry_c.scope_data, DefaultScope::Request.into());
    }

    #[test]
    #[traced_test]
    fn test_registry_extend_entry_can_inject_entry_from_nested_extend() {
        let registry = registry! {
            provide(DefaultScope::App, inst_a),
            extend(
                registry! {
                    provide(DefaultScope::Session, inst_b_with_c),
                    extend(
                        registry! {
                            provide(DefaultScope::Runtime, inst_c),
                        },
                    ),
                },
            ),
        };

        let entry_b = registry.get(&TypeInfo::of::<((), ())>()).unwrap();

        assert_eq!(registry.entries.len(), 4);
        assert!(entry_b
            .dependencies
            .iter()
            .any(|dependency| dependency.type_info == TypeInfo::of::<((), (), ())>()));
    }

    #[test]
    #[traced_test]
    fn test_registry_extend_later_registry_overrides_duplicate_entry() {
        let registry = registry! {
            extend(
                registry! {
                    provide(
                        DefaultScope::App,
                        inst_a,
                        config = Config { cache_provides: false },
                    ),
                },
                registry! {
                    provide(DefaultScope::Request, inst_a),
                },
            ),
        };

        let entry = registry.get(&TypeInfo::of::<()>()).unwrap();

        assert!(entry.config.cache_provides);
        assert_eq!(entry.scope_data, DefaultScope::Request.into());
    }

    #[test]
    #[traced_test]
    fn test_registry_extend_merges_duplicate_entries_left_to_right() {
        let registry = registry! {
            extend(
                registry! {
                    provide(
                        DefaultScope::App,
                        inst_a,
                        config = Config { cache_provides: false },
                    ),
                },
                registry! {
                    provide(DefaultScope::Session, inst_a),
                },
                registry! {
                    provide(
                        DefaultScope::Request,
                        inst_a,
                        config = Config { cache_provides: false },
                    ),
                },
            ),
        };

        let entry = registry.get(&TypeInfo::of::<()>()).unwrap();

        assert!(!entry.config.cache_provides);
        assert_eq!(entry.scope_data, DefaultScope::Request.into());
    }

    #[test]
    #[traced_test]
    fn test_registry_extend_overrides_duplicate_entry_from_previous_macro_invocation() {
        let registry = registry! {
            provide(
                DefaultScope::App,
                inst_a,
                config = Config { cache_provides: false },
            ),
            extend(
                registry! {
                    provide(DefaultScope::Request, inst_a),
                },
            ),
        };

        let entry = registry.get(&TypeInfo::of::<()>()).unwrap();

        assert!(entry.config.cache_provides);
        assert_eq!(entry.scope_data, DefaultScope::Request.into());
    }
}
