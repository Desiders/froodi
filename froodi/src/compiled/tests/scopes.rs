extern crate std;

use crate as scope_api;
#[cfg(feature = "async")]
use crate::{async_impl::Container as AsyncContainer, async_registry as dynamic_async_registry, compiled_async_registry as async_registry};
use crate::{
    compiled_registry as registry, context, instance, registry as dynamic_registry, runtime, Container, Context, DefaultScope, Inject,
    InjectTransient, InstantiateErrorKind, RuntimeDependency, Scope, ScopeData, Scopes, StaticScope, TypeInfo,
};
use alloc::string::{String, ToString as _};
use core::cell::Cell;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[path = "../../../tests/ui/compiled/support/scopes.rs"]
mod scopes;
use scopes::{App, EqualApp, Request, Root, RuntimeScope, Transaction};

fn service(Inject(value): Inject<u32>) -> Result<String, InstantiateErrorKind> {
    Ok(value.to_string())
}

#[test]
fn static_metadata_supplies_runtime_scope_methods() {
    fn check<S: StaticScope>(scope: S) {
        assert_eq!(scope.name(), S::DATA.name);
        assert_eq!(scope.priority(), S::DATA.priority);
        assert_eq!(scope.is_skipped_by_default(), S::DATA.is_skipped_by_default);
        let data: ScopeData = scope.into();
        assert_eq!(data, S::DATA);
    }

    check(Root);
    check(App);
    check(Request);
    check(Transaction);
    check(EqualApp);
}

#[test]
fn same_wider_and_equal_priority_dependencies_match_runtime() {
    type AppAlias = App;
    let alias: AppAlias = App;

    let container = Container::new(registry! {
        provide(alias, instance(7u32)),
        provide(App, service),
    });
    assert_eq!(&*container.get::<String>().unwrap(), "7");
    let native = dynamic_registry! { provide(DefaultScope::App, instance(7u32)), provide(DefaultScope::App, service) };
    assert!(native.validate().is_ok());

    let container = Container::new(registry! {
        provide(App, instance(8u32)),
        provide(Request, service),
        provide(Transaction, |value: Inject<String>, fresh: InjectTransient<u32>| Ok::<_, InstantiateErrorKind>((value.0.len(), fresh.0))),
    })
    .enter()
    .with_scope(Request)
    .build()
    .unwrap()
    .enter()
    .with_scope(Transaction)
    .build()
    .unwrap();
    assert_eq!(*container.get::<(usize, u32)>().unwrap(), (1, 8));
    let native = dynamic_registry! { provide(DefaultScope::App, instance(8u32)), provide(DefaultScope::Request, service) };
    assert!(native.validate().is_ok());

    let container = Container::new(registry! { provide(EqualApp, instance(9u32)), provide(App, service) });
    assert_eq!(&*container.get::<String>().unwrap(), "9");
    let mut native = dynamic_registry! { provide(DefaultScope::App, instance(9u32)), provide(DefaultScope::App, service) };
    native.entries.get_mut(&TypeInfo::of::<u32>()).unwrap().scope_data = EqualApp::DATA;
    assert!(native.validate().is_ok());
}

#[test]
fn generic_bounds_determine_scope_capability() {
    fn dynamic<S: Scope + Scopes<3, Scope = ScopeData>>(scope: S) -> Container {
        Container::new(registry! { provide(scope, service), provide(Request, instance(7u32)) })
    }

    fn static_valid<S: StaticScope + Scopes<3, Scope = ScopeData>>(scope: S) -> Container {
        Container::new(registry! { provide(scope, service), provide(App, instance(7u32)) })
    }

    assert!(catch_unwind(|| dynamic(App)).is_err());
    assert_eq!(
        &*static_valid(Request)
            .enter()
            .with_scope(Request)
            .build()
            .unwrap()
            .get::<String>()
            .unwrap(),
        "7"
    );
}

#[test]
fn runtime_values_and_default_scope_keep_runtime_validation() {
    assert!(catch_unwind(|| Container::new(registry! {
        provide(RuntimeScope(1), service), provide(RuntimeScope(3), instance(7u32)),
    }))
    .is_err());
    assert!(catch_unwind(|| Container::new(registry! {
        provide(DefaultScope::App, service), provide(DefaultScope::Request, instance(7u32)),
    }))
    .is_err());
    assert!(catch_unwind(|| Container::new(registry! {
        provide(App, service), provide(RuntimeScope(3), instance(7u32)),
    }))
    .is_err());
    assert!(catch_unwind(|| Container::new(registry! {
        provide(RuntimeScope(1), service), provide(Request, instance(7u32)),
    }))
    .is_err());
    let container = Container::new(registry! { provide(RuntimeScope(1), instance(7u32)), provide(Request, service) });
    assert_eq!(
        &*container.enter().with_scope(Request).build().unwrap().get::<String>().unwrap(),
        "7"
    );
    assert!(catch_unwind(|| Container::new(registry! {
        provide(App, |_: Inject<u32>, _: RuntimeDependency<()>| Ok::<_, InstantiateErrorKind>(String::new())),
        provide(Request, instance(7u32)),
    }))
    .is_err());
}

#[test]
fn native_validation_rejects_equivalent_invalid_static_relationships() {
    assert!(
        catch_unwind(|| dynamic_registry! { provide(DefaultScope::App, service), provide(DefaultScope::Request, instance(7u32)) }).is_err()
    );
    assert!(catch_unwind(|| dynamic_registry! {
        provide(DefaultScope::App, |_: InjectTransient<u32>| Ok::<_, InstantiateErrorKind>(String::new())),
        provide(DefaultScope::Request, instance(7u32)),
    })
    .is_err());
}

#[test]
fn final_typed_composition_and_erasure_use_the_effective_selection() {
    let fragment = registry! { provide(Request, service) };
    let container = Container::new(registry! { extend(fragment), provide(App, instance(7u32)) });
    assert_eq!(
        &*container.enter().with_scope(Request).build().unwrap().get::<String>().unwrap(),
        "7"
    );

    let fragment = registry! { provide(App, service), provide(Request, instance(7u32)) };
    let native = dynamic_registry! { provide(DefaultScope::App, instance(9u32)) };
    let container = Container::new(registry! { extend(native, fragment) });
    assert_eq!(&*container.get::<String>().unwrap(), "9");

    let erased = registry! { provide(App, service), provide(Request, instance(7u32)) }.into_registry();
    assert!(erased.validate().is_err());
    let container = Container::new(dynamic_registry! {
        extend(erased, dynamic_registry! { provide(DefaultScope::App, instance(11u32)) }),
    });
    assert_eq!(&*container.get::<String>().unwrap(), "11");

    let fragment = registry! { provide(App, service), provide(App, instance(7u32)) };
    let replacement = dynamic_registry! { provide(DefaultScope::Request, instance(11u32)) };
    assert!(catch_unwind(AssertUnwindSafe(|| Container::new(registry! { extend(fragment, replacement) }))).is_err());
}

#[test]
fn imports_and_context_do_not_reuse_static_provider_scope_guarantees() {
    let fragment = registry! { provide(App, service), provide(App, runtime::<u32>()) };
    let native = dynamic_registry! { provide(DefaultScope::Request, instance(7u32)) };
    assert!(catch_unwind(AssertUnwindSafe(|| Container::new(registry! { extend(fragment, native) }))).is_err());

    let container = Container::new(registry! { provide(Request, service), provide(Request, context::<u32>()) });
    let mut context = Context::new();
    context.insert(7u32);
    let container = container.enter().with_scope(Request).with_context(context).build().unwrap();
    assert_eq!(&*container.get::<String>().unwrap(), "7");
}

#[test]
fn skipped_root_and_explicit_start_scope_remain_runtime_concerns() {
    let container = Container::new_compiled_with_start_scope(
        registry! {
            provide(Root, instance(7u32)), provide(Request, service),
        },
        Root,
    )
    .enter()
    .with_scope(App)
    .build()
    .unwrap()
    .enter()
    .with_scope(Request)
    .build()
    .unwrap();
    assert_eq!(&*container.get::<String>().unwrap(), "7");
}

#[test]
fn borrowed_dynamic_scope_is_erased_before_registry_use() {
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct Borrowed<'a>(&'a u8);

    impl Scope for Borrowed<'_> {
        fn name(&self) -> &'static str {
            "borrowed"
        }

        fn priority(&self) -> u8 {
            *self.0
        }
    }

    impl From<Borrowed<'_>> for ScopeData {
        fn from(scope: Borrowed<'_>) -> Self {
            Self {
                priority: scope.priority(),
                name: scope.name(),
                is_skipped_by_default: false,
            }
        }
    }

    impl Scopes<3> for Borrowed<'_> {
        type Scope = ScopeData;

        fn all() -> (Self::Scope, [Self::Scope; 3]) {
            Root::all()
        }
    }

    let typed;
    {
        let priority = 1;
        typed = registry! { provide(Borrowed(&priority), instance(7u32)) };
    }
    assert_eq!(*Container::new(typed).get::<u32>().unwrap(), 7);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_frontend_and_erasure_preserve_scope_selection() {
    let fragment = async_registry! { provide(App, async || Ok::<_, InstantiateErrorKind>(7u32)) };
    let container = AsyncContainer::new(async_registry! {
        provide(Request, async |value: Inject<u32>| Ok::<_, InstantiateErrorKind>(value.0.to_string())),
        extend(fragment),
    })
    .enter()
    .with_scope(Request)
    .build()
    .unwrap();
    assert_eq!(&*container.get::<String>().await.unwrap(), "7");

    let erased = async_registry! {
        provide(App, async |value: Inject<u32>| Ok::<_, InstantiateErrorKind>(value.0.to_string())),
        provide(Request, async || Ok::<_, InstantiateErrorKind>(7u32)),
    }
    .into_async_registry();
    let replacement = dynamic_async_registry! { provide(DefaultScope::App, async || Ok::<_, InstantiateErrorKind>(9u32)) };
    let container = AsyncContainer::new(async_registry! { extend(erased, replacement) });
    assert_eq!(&*container.get::<String>().await.unwrap(), "9");

    assert!(catch_unwind(|| dynamic_async_registry! {
        provide(DefaultScope::App, async |value: Inject<u32>| Ok::<_, InstantiateErrorKind>(value.0.to_string())),
        provide(DefaultScope::Request, async || Ok::<_, InstantiateErrorKind>(7u32)),
    })
    .is_err());
}

#[test]
fn scope_groups_evaluate_and_consume_the_scope_once() {
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct MoveScope(String);

    impl From<MoveScope> for ScopeData {
        fn from(_: MoveScope) -> Self {
            MoveScope::DATA
        }
    }

    impl Scopes<3> for MoveScope {
        type Scope = ScopeData;

        fn all() -> (Self::Scope, [Self::Scope; 3]) {
            Root::all()
        }
    }

    impl StaticScope for MoveScope {
        const DATA: ScopeData = App::DATA;
    }

    let evaluations = Cell::new(0);
    let owned = String::from("move once");
    let container = Container::new(registry! {
        scope({ evaluations.set(evaluations.get() + 1); MoveScope(owned) }) [
            provide(instance(7u32)),
            provide(service),
        ],
    });
    assert_eq!(evaluations.get(), 1);
    assert_eq!(&*container.get::<String>().unwrap(), "7");
}

#[test]
fn inconsistent_static_metadata_is_rejected_when_scope_values_are_evaluated() {
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct BadScope;

    impl From<BadScope> for ScopeData {
        fn from(_: BadScope) -> Self {
            Self {
                priority: 3,
                name: "bad",
                is_skipped_by_default: false,
            }
        }
    }

    impl Scopes<3> for BadScope {
        type Scope = ScopeData;

        fn all() -> (Self::Scope, [Self::Scope; 3]) {
            Root::all()
        }
    }

    impl StaticScope for BadScope {
        const DATA: ScopeData = App::DATA;
    }

    let error = catch_unwind(|| registry! { provide(BadScope, instance(7u32)) }).err().unwrap();
    let message = error.downcast_ref::<String>().unwrap();
    assert!(message.contains("StaticScope metadata differs from Scope conversion"), "{message}");
}

#[test]
fn custom_scope_family_matches_native_hierarchy_and_priority_rules() {
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Family {
        Root,
        App,
        Request,
    }

    impl Scope for Family {
        fn name(&self) -> &'static str {
            match self {
                Self::Root => "custom root",
                Self::App => "custom app",
                Self::Request => "custom request",
            }
        }

        fn priority(&self) -> u8 {
            *self as u8
        }

        fn is_skipped_by_default(&self) -> bool {
            matches!(self, Self::Root)
        }
    }

    impl From<Family> for ScopeData {
        fn from(scope: Family) -> Self {
            Self {
                priority: scope.priority(),
                name: scope.name(),
                is_skipped_by_default: scope.is_skipped_by_default(),
            }
        }
    }

    impl Scopes<2> for Family {
        type Scope = Self;

        fn all() -> (Self, [Self; 2]) {
            (Self::Root, [Self::App, Self::Request])
        }
    }

    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct Static<const PRIORITY: u8>;

    impl<const PRIORITY: u8> From<Static<PRIORITY>> for ScopeData {
        fn from(_: Static<PRIORITY>) -> Self {
            Static::<PRIORITY>::DATA
        }
    }

    impl<const PRIORITY: u8> Scopes<2> for Static<PRIORITY> {
        type Scope = Family;

        fn all() -> (Family, [Family; 2]) {
            Family::all()
        }
    }

    impl<const PRIORITY: u8> StaticScope for Static<PRIORITY> {
        const DATA: ScopeData = ScopeData {
            priority: PRIORITY,
            name: match PRIORITY {
                0 => "custom root",
                1 => "custom app",
                _ => "custom request",
            },
            is_skipped_by_default: PRIORITY == 0,
        };
    }

    let container = Container::new(registry! {
        provide(Static::<1>, instance(7u32)),
        scope(Static::<2>) [ provide(service) ],
    });
    assert_eq!(&*container.enter_build().unwrap().get::<String>().unwrap(), "7");
    let native = dynamic_registry! { provide(Family::App, instance(7u32)), provide(Family::Request, service) };
    assert!(native.validate().is_ok());
    assert!(catch_unwind(|| dynamic_registry! { provide(Family::App, service), provide(Family::Request, instance(7u32)) }).is_err());
}
