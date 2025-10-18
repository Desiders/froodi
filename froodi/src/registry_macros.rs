use frunk::{hlist, hlist::Selector, HList};

use crate::{
    finalizer::BoxedCloneFinalizer, instantiator::BoxedCloneInstantiator, scope::ScopeData, Config, InstantiateErrorKind, ResolveErrorKind,
};

#[derive(Clone)]
pub(crate) struct InstantiatorData {
    pub(crate) instantiator: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,
    pub(crate) finalizer: Option<BoxedCloneFinalizer>,
    pub(crate) config: Config,
    pub(crate) scope: ScopeData,
}

#[derive(Clone)]
pub struct Registry<H> {
    pub entries: H,
}

impl<H> Registry<H> {
    pub fn get_entry<T, Index>(&self) -> &InstantiatorData
    where
        H: Selector<InstantiatorData, Index>,
    {
        self.entries.get()
    }
}

#[macro_export]
macro_rules! registry {
    (
        $(
            scope($scope:ident) [ $( $entries:tt )* ]
        ),* $(,)?
    ) => {{
        $(
            {
                tracing::debug!("Parsed scope: {}", stringify!($scope));
                $crate::registry_internal! { @entries $scope [ $($entries)* ] }
            };
        )*

        ()
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! registry_internal {
    // === Base case ===
    // Example: registry_internal! { @entries App [] }
    (@entries $scope:ident []) => {};

    // === Single identifier entries ===
    // Example: registry_internal! { @entries App [ inst_a ] }
    (@entries $scope:ident [ $inst:ident $(, $($rest:tt)*)? ]) => {
        $crate::registry_internal! { @entry $scope, $inst }
        $crate::registry_internal! { @entries $scope [ $($($rest)*)? ] }
    };

    // === Parenthesized entries ===
    // Example: registry_internal! { @entries App [ (inst_b), (inst_c, config = cfg) ] }
    (@entries $scope:ident [ ( $($entry:tt)+ ) $(, $($rest:tt)*)? ]) => {
        $crate::registry_internal! { @entry $scope, $($entry)+ }
        $crate::registry_internal! { @entries $scope [ $($($rest)*)? ] }
    };

    // === Entry with no options ===
    // Example: registry_internal! { @entry App, inst_a }
    (@entry $scope:ident, $inst:ident) => {
        $crate::registry_internal! { @entry_with_options $scope, $inst, config = None, finalizer = None }
    };

    // === Entry with config only ===
    // Example: registry_internal! { @entry App, (inst_c, config = cfg_c) }
    (@entry $scope:ident, $inst:ident, config = $cfg:expr) => {
        $crate::registry_internal! { @entry_with_options $scope, $inst, config = Some($cfg), finalizer = None }
    };

    // === Entry with finalizer only ===
    // Example: registry_internal! { @entry App, (inst_d, finalizer = fin_d) }
    (@entry $scope:ident, $inst:ident, finalizer = $fin:expr) => {
        $crate::registry_internal! { @entry_with_options $scope, $inst, config = None, finalizer = Some($fin) }
    };

    // === Entry with config + finalizer (config first) ===
    // Example: registry_internal! { @entry App, (inst_e, config = cfg_e, finalizer = fin_e) }
    (@entry $scope:ident, $inst:ident, config = $cfg:expr, finalizer = $fin:expr) => {
        $crate::registry_internal! { @entry_with_options $scope, $inst, config = Some($cfg), finalizer = Some($fin) }
    };

    // === Entry with finalizer + config (finalizer first) ===
    // Example: registry_internal! { @entry App, (inst_f, finalizer = fin_f, config = cfg_f) }
    (@entry $scope:ident, $inst:ident, finalizer = $fin:expr, config = $cfg:expr) => {
        $crate::registry_internal! { @entry_with_options $scope, $inst, config = Some($cfg), finalizer = Some($fin) }
    };

    // === Unified final expansion ===
    // Example: registry_internal! { @entry_with_options App, inst_a, config = Some(cfg_a), finalizer = None }
    (@entry_with_options $scope:ident, $inst:ident, config = $cfg:expr, finalizer = $fin:expr) => {{
        let data = $crate::registry_macros::InstantiatorData {
            instantiator: $crate::instantiator::boxed_instantiator($inst),
            finalizer: match $fin {
                Some(finalizer) => Some($crate::finalizer::boxed_finalizer_factory(finalizer)),
                None => None,
            },
            config: match $cfg {
                Some(config) => config,
                None => $crate::Config::default(),
            },
            scope: $crate::DefaultScope::$scope,
        };

        tracing::debug!(
            "entry: {}, scope: {}, config: {}, finalizer: {}",
            stringify!($inst),
            stringify!($scope),
            stringify!($cfg),
            stringify!($fin)
        );
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

    use crate::{Config, DefaultScope, InstantiateErrorKind};

    fn scope_a() -> DefaultScope {
        DefaultScope::App
    }

    fn inst_a() -> Result<(), InstantiateErrorKind> {
        Ok(())
    }
    fn inst_b() -> Result<(), InstantiateErrorKind> {
        Ok(())
    }
    fn inst_c() -> Result<(), InstantiateErrorKind> {
        Ok(())
    }
    fn inst_d() -> Result<(), InstantiateErrorKind> {
        Ok(())
    }
    fn inst_e() -> Result<(), InstantiateErrorKind> {
        Ok(())
    }
    fn inst_f() -> Result<(), InstantiateErrorKind> {
        Ok(())
    }

    #[test]
    #[traced_test]
    fn test_entry_simple_ident() {
        registry_internal! { @entries scope [ inst_a ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_tuple_single() {
        registry_internal! { @entries scope [ (inst_a) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_config() {
        registry_internal! { @entries scope [ (inst_a, config = Config::default()) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_finalizer() {
        registry_internal! { @entries scope [ (inst_a, finalizer = |_: RcThreadSafety<()>| {}) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_config_and_finalizer() {
        registry_internal! { @entries scope [ (inst_a, config = Config::default(), finalizer = |_: RcThreadSafety<()>| {}) ] };
    }

    #[test]
    #[traced_test]
    fn test_entry_with_finalizer_and_config_swapped() {
        registry_internal! { @entries scope [ (inst_a, finalizer = |_: RcThreadSafety<()>| {}, config = Config::default()) ] };
    }

    #[test]
    #[traced_test]
    fn test_multiple_entries() {
        registry_internal! {
            @entries scope [
                inst_a,
                (inst_b),
                (inst_c, config = Config::default()),
                (inst_d, finalizer = |_: RcThreadSafety<()>| {}),
                (inst_e, config = Config::default(), finalizer = |_: RcThreadSafety<()>| {}),
            ]
        };
    }

    #[test]
    #[traced_test]
    fn test_trailing_comma_and_spaces() {
        registry_internal! {
            @entries scope [
                (inst_x, config = Config::default(), finalizer = |_: RcThreadSafety<()>| {}),
            ]
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_single_scope_basic() {
        registry! {
            scope(App) [
                inst_a,
                (inst_b),
                (inst_c, config = cfg_c),
                (inst_d, finalizer = fin_d),
                (inst_e, config = cfg_e, finalizer = fin_e),
            ],
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_multiple_scopes() {
        registry! {
            scope(App) [
                inst_a,
                (inst_b),
            ],
            scope(Request) [
                (inst_r1, config = cfg_r1),
                (inst_r2, finalizer = fin_r2),
            ],
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_empty_scope() {
        registry! {
            scope(Empty) []
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_trailing_commas_and_spacing() {
        registry! {
            scope(App)[
                inst_a,
                (inst_b , config = cfg_b , finalizer = fin_b)
            ]
            , scope(Request)[ (inst_x) , ]
        };
    }

    #[test]
    #[traced_test]
    fn test_registry_mixed_entries() {
        registry! {
            scope(Mixed) [
                inst_a,
                (inst_b),
                (inst_c, config = cfg_c),
                (inst_d, finalizer = fin_d),
                (inst_e, config = cfg_e, finalizer = fin_e),
                (inst_f, finalizer = fin_f, config = cfg_f),
            ]
        };
    }
}
