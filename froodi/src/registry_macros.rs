use core::any::TypeId;

use crate::{
    finalizer::BoxedCloneFinalizer, instantiator::BoxedCloneInstantiator, scope::ScopeData, utils::hlist::HListFind, Config,
    InstantiateErrorKind, ResolveErrorKind,
};

#[derive(Clone)]
pub(crate) struct InstantiatorData {
    pub(crate) instantiator: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>,
    pub(crate) finalizer: Option<BoxedCloneFinalizer>,
    pub(crate) config: Config,
    pub(crate) scope: ScopeData,
    pub(crate) type_id: TypeId,
}

#[derive(Clone)]
pub struct Registry<H> {
    pub entries: H,
}

impl<H> Registry<H> {
    pub fn get<Dep>(&self) -> Option<&InstantiatorData>
    where
        Dep: 'static,
        H: HListFind<InstantiatorData, TypeId>,
    {
        self.entries.get(TypeId::of::<Dep>())
    }
}

#[macro_export]
macro_rules! registry {
    (
        scope($scope:expr) [ $( $entries:tt )* ]
        $(, scope($rest_scope:expr) [ $( $rest_entries:tt )* ] )* $(,)?
    ) => {{
        let entries = frunk::hlist![
            $crate::registry_internal! { @entries scope($scope) [ $($entries)* ] },
            $(
                $crate::registry_internal! { @entries scope($rest_scope) [ $($rest_entries)* ] }
            ),*
        ];

        $crate::registry_macros::Registry {
            entries,
        }
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! registry_internal {
    // === Empty entries ===
    (@entries scope($scope:expr) []) => {{
        frunk::hlist::HNil
    }};
    (@entries scope($scope:expr) [ provide( $($entry:tt)+ ) $(, $($rest:tt)*)? ]) => {
        frunk::hlist![
            $crate::registry_internal! { @entry scope($scope), $($entry)+ },
            $(
                $crate::registry_internal! { @entries scope($scope) [ $($rest)* ] }
            )?
        ]
    };

    // === Entry ===
    (@entry scope($scope:expr), $inst:expr) => {{
        $crate::registry_internal! { @entry_with_options scope($scope), $inst, config = None, finalizer = None::<fn(_)> }
    }};
    // === Entry with config ===
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr) => {{
        $crate::registry_internal! { @entry_with_options scope($scope), $inst, config = Some($cfg), finalizer = None::<fn(_)> }
    }};
    // === Entry with finalizer ===
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr) => {{
        $crate::registry_internal! { @entry_with_options scope($scope), $inst, config = None, finalizer = Some($fin) }
    }};
    // === Entry with config (first) and finalizer ===
    (@entry scope($scope:expr), $inst:expr, config = $cfg:expr, finalizer = $fin:expr) => {{
        $crate::registry_internal! { @entry_with_options scope($scope), $inst, config = Some($cfg), finalizer = Some($fin) }
    }};
    // === Entry with finalizer (first) and config ===
    (@entry scope($scope:expr), $inst:expr, finalizer = $fin:expr, config = $cfg:expr) => {{
        $crate::registry_internal! { @entry_with_options scope($scope), $inst, config = Some($cfg), finalizer = Some($fin) }
    }};

    // === Entry with config and finalizer ===
    (@entry_with_options scope($scope:expr), $inst:expr, config = $cfg:expr, finalizer = $fin:expr) => {{
        #[inline]
        fn impl_<Inst, Deps, Fin>(inst: Inst, fin: Option<Fin>) -> $crate::registry_macros::InstantiatorData
        where
            Inst: $crate::instantiator::Instantiator<Deps, Error = InstantiateErrorKind>
                + $crate::utils::thread_safety::SendSafety
                + $crate::utils::thread_safety::SyncSafety,
            Inst::Provides:
                  $crate::utils::thread_safety::SendSafety
                + $crate::utils::thread_safety::SyncSafety,
            Deps: $crate::dependency_resolver::DependencyResolver<Error = $crate::ResolveErrorKind>,
            Fin: $crate::finalizer::Finalizer<Inst::Provides>
                + $crate::utils::thread_safety::SendSafety
                + $crate::utils::thread_safety::SyncSafety,
        {
            $crate::registry_macros::InstantiatorData {
                type_id: core::any::TypeId::of::<Inst::Provides>(),
                instantiator: $crate::instantiator::boxed_instantiator(inst),
                finalizer: match fin {
                    Some(finalizer) => Some($crate::finalizer::boxed_finalizer_factory(finalizer)),
                    None => None,
                },
                config: match $cfg {
                    Some(config) => config,
                    None => $crate::Config::default(),
                },
                scope: $scope.into(),
            }
        }
        impl_($inst, $fin)
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

    use crate::{utils::thread_safety::RcThreadSafety, Config, DefaultScope, Inject, InstantiateErrorKind};

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
            scope(None) []
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

        assert!(registry.get::<()>().is_some());
        assert!(registry.get::<((), ())>().is_some());
        assert!(registry.get::<((), (), ())>().is_some());
        assert!(registry.get::<((), (), (), ())>().is_some());
        assert!(registry.get::<((), (), (), (), ())>().is_some());
        assert!(registry.get::<((), (), (), (), (), ())>().is_some());
        assert!(registry.get::<((), (), (), (), (), (), ())>().is_none());
    }
}
