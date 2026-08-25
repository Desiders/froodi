//! A user-defined `Scope` driven entirely through the public API, so the scope type and the `N` of
//! `Scopes<N>` that `registry!` infers are exercised with something other than `DefaultScope`.
#![no_std]

extern crate alloc;

use froodi::{registry, Container, InstantiateErrorKind, ResolveErrorKind, Scope, ScopeData, Scopes};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum MyScope {
    Boot,
    Work,
    Task,
}

impl From<MyScope> for ScopeData {
    fn from(scope: MyScope) -> Self {
        Self {
            priority: scope.priority(),
            name: scope.name(),
            is_skipped_by_default: scope.is_skipped_by_default(),
        }
    }
}

impl Scope for MyScope {
    fn name(&self) -> &'static str {
        match self {
            MyScope::Boot => "boot",
            MyScope::Work => "work",
            MyScope::Task => "task",
        }
    }

    fn priority(&self) -> u8 {
        *self as u8
    }

    fn is_skipped_by_default(&self) -> bool {
        matches!(self, MyScope::Boot)
    }
}

// Deliberately not 5: `N` must come from this impl, not from `DefaultScope`.
impl Scopes<2> for MyScope {
    type Scope = Self;

    fn all() -> (Self, [Self; 2]) {
        (MyScope::Boot, [MyScope::Work, MyScope::Task])
    }
}

#[derive(Debug)]
struct Wide;
#[derive(Debug)]
struct Narrow;

fn wide() -> Result<Wide, InstantiateErrorKind> {
    Ok(Wide)
}
fn narrow() -> Result<Narrow, InstantiateErrorKind> {
    Ok(Narrow)
}

fn container() -> Container {
    Container::new(registry! {
        scope(MyScope::Work) [ provide(wide) ],
        provide(MyScope::Task, narrow),
    })
}

#[test]
fn resolves_an_entry_at_the_start_scope() {
    assert!(container().get::<Wide>().is_ok());
}

#[test]
fn reports_the_custom_scope_names_when_an_entry_is_out_of_reach() {
    match container().get::<Narrow>() {
        Err(ResolveErrorKind::NoAccessible {
            expected_scope_data,
            actual_scope_data,
        }) => {
            assert_eq!(expected_scope_data.name, "task");
            assert_eq!(actual_scope_data.name, "work");
        }
        other => panic!("expected NoAccessible, got {other:?}"),
    }
}

#[test]
fn a_child_container_reaches_the_narrower_scope() {
    let child = container().enter().with_scope(MyScope::Task).build().unwrap();

    assert!(child.get::<Narrow>().is_ok());
}

#[test]
fn start_scope_can_be_chosen_explicitly() {
    let container = Container::new_with_start_scope(registry! { scope(MyScope::Task) [ provide(narrow) ] }, MyScope::Task);

    assert!(container.get::<Narrow>().is_ok());
}

#[test]
#[should_panic]
fn a_wider_entry_may_not_depend_on_a_narrower_one() {
    registry! {
        scope(MyScope::Work) [ provide(|froodi::InjectTransient(_): froodi::InjectTransient<Narrow>| Ok(Wide)) ],
        scope(MyScope::Task) [ provide(narrow) ],
    };
}
