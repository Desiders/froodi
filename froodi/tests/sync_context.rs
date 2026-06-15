#![no_std]

extern crate alloc;

use froodi::utils::thread_safety::RcThreadSafety;
use froodi::{
    registry, Config, Container, Context,
    DefaultScope::{Action, App, Request, Step},
    Inject,
};

// `Provided` is never registered in any registry; resolvable ONLY via Context.
#[derive(PartialEq, Eq, Debug)]
struct Provided(u32);

#[derive(PartialEq, Eq, Debug)]
struct Marker(u32);

struct Other(u32);

fn registry_without_provided() -> froodi::Registry {
    registry! {
        scope(App) [
            provide(|| Ok(Other(1))),
        ],
        scope(Request) [
            provide(|Inject(o): Inject<Other>| Ok(Other(o.0 + 1))),
        ],
        scope(Action) [
            provide(|| Ok(Other(100)), config = Config::default()),
        ],
    }
}

#[test]
fn test_context_value_resolvable_without_provider() {
    // First get from a plain container MUST fail, proving `Provided` has no instantiator.
    let plain = Container::new(registry_without_provided());
    assert!(matches!(
        plain.get::<Provided>(),
        Err(froodi::ResolveErrorKind::NoInstantiator { .. })
    ));

    let app = Container::new(registry_without_provided());

    let mut ctx = Context::new();
    let prev = ctx.insert(Provided(42));
    assert!(prev.is_none());

    let child = app.enter().with_context(ctx).build().unwrap();

    let got = child.get::<Provided>().unwrap();
    assert_eq!(got.0, 42);
    assert_eq!(*got, Provided(42));
}

#[test]
fn test_context_value_shared_same_arc() {
    let app = Container::new(registry_without_provided());

    let mut ctx = Context::new();
    ctx.insert(Provided(7));

    let child = app.enter().with_context(ctx).build().unwrap();

    let first = child.get::<Provided>().unwrap();
    let second = child.get::<Provided>().unwrap();

    assert_eq!(first.0, 7);
    assert_eq!(second.0, 7);
    // Context entries are shared: same allocation across gets.
    assert!(RcThreadSafety::ptr_eq(&first, &second));
}

#[test]
fn test_context_propagates_to_grandchild() {
    let app = Container::new(registry_without_provided());

    let mut ctx = Context::new();
    ctx.insert(Provided(99));

    let child = app.enter().with_context(ctx).build().unwrap();
    let child_val = child.get::<Provided>().unwrap();
    assert_eq!(child_val.0, 99);

    // grandchild descends without re-supplying context.
    let grandchild = child.clone().enter_build().unwrap();
    let grand_val = grandchild.get::<Provided>().unwrap();
    assert_eq!(grand_val.0, 99);

    assert!(RcThreadSafety::ptr_eq(&child_val, &grand_val));
}

#[test]
fn test_context_insert_returns_previous_on_overwrite() {
    let mut ctx = Context::new();

    let first = ctx.insert(Marker(1));
    assert!(first.is_none());

    let prev = ctx.insert(Marker(2));
    let prev = prev.expect("overwrite should return previous value");
    assert_eq!(prev.0, 1);
    assert_eq!(*prev, Marker(1));

    let prev2 = ctx.insert(Marker(3)).expect("overwrite should return previous value");
    assert_eq!(prev2.0, 2);

    // Distinct type is independent despite an existing Marker.
    let other_first = ctx.insert(Provided(0));
    assert!(other_first.is_none());
}

#[test]
fn test_context_default_is_empty() {
    let mut ctx = Context::default();
    assert!(ctx.insert(Marker(5)).is_none());
    assert!(ctx.insert(Provided(5)).is_none());

    assert!(ctx.insert(Marker(6)).is_some());
}

#[test]
fn test_with_scope_and_context_injects_context() {
    let app = Container::new(registry_without_provided());

    let mut ctx = Context::new();
    ctx.insert(Provided(123));

    let action = app.enter().with_scope(Action).with_context(ctx).build().unwrap();

    let got = action.get::<Provided>().unwrap();
    assert_eq!(got.0, 123);

    // Resolving an Action-provided dep confirms the descend landed on Action.
    let other = action.get::<Other>().unwrap();
    assert_eq!(other.0, 100);

    let got2 = action.get::<Provided>().unwrap();
    assert!(RcThreadSafety::ptr_eq(&got, &got2));
}

#[test]
fn test_with_context_then_with_scope_injects_context() {
    let app = Container::new(registry_without_provided());

    let mut ctx = Context::new();
    ctx.insert(Provided(456));

    let step_capable = app.enter().with_context(ctx).with_scope(Action).build().unwrap();

    let got = step_capable.get::<Provided>().unwrap();
    assert_eq!(got.0, 456);

    let deeper = step_capable.clone().enter().with_scope(Step).build().unwrap();
    let deep_got = deeper.get::<Provided>().unwrap();
    assert_eq!(deep_got.0, 456);
    assert!(RcThreadSafety::ptr_eq(&got, &deep_got));
}
