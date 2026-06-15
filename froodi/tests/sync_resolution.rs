#![no_std]

extern crate alloc;

use alloc::string::ToString as _;
use core::sync::atomic::{AtomicU8, Ordering};
use froodi::{
    instance, registry,
    utils::thread_safety::RcThreadSafety,
    Config, Container,
    DefaultScope::{App, Request},
    Inject, InjectTransient, ResolveErrorKind,
};

#[test]
fn test_get_caches_single_instance() {
    struct Singleton(u8);

    let calls = RcThreadSafety::new(AtomicU8::new(0));

    let container = Container::new(registry! {
        scope(App) [
            provide({
                let calls = calls.clone();
                move || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(Singleton(42))
                }
            }),
        ]
    });

    let first = container.get::<Singleton>().unwrap();
    let second = container.get::<Singleton>().unwrap();
    let third = container.get::<Singleton>().unwrap();

    assert!(RcThreadSafety::ptr_eq(&first, &second));
    assert!(RcThreadSafety::ptr_eq(&second, &third));
    assert_eq!(first.0, 42);

    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn test_get_transient_fresh_each_call() {
    struct Counted(u8);

    let calls = RcThreadSafety::new(AtomicU8::new(0));

    let container = Container::new(registry! {
        scope(App) [
            provide({
                let calls = calls.clone();
                move || {
                    let prev = calls.fetch_add(1, Ordering::SeqCst);
                    Ok(Counted(prev + 1))
                }
            }),
        ]
    });

    let a = container.get_transient::<Counted>().unwrap();
    let b = container.get_transient::<Counted>().unwrap();
    let c = container.get_transient::<Counted>().unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 3);

    assert_eq!(a.0, 1);
    assert_eq!(b.0, 2);
    assert_eq!(c.0, 3);
    assert_ne!(a.0, b.0);
    assert_ne!(b.0, c.0);
}

#[test]
fn test_scoped_dependency_is_shared() {
    struct Leaf(u8);
    struct Mid(RcThreadSafety<Leaf>);
    struct Top(RcThreadSafety<Leaf>, RcThreadSafety<Mid>);

    let leaf_calls = RcThreadSafety::new(AtomicU8::new(0));

    let container = Container::new(registry! {
        scope(App) [
            provide({
                let leaf_calls = leaf_calls.clone();
                move || {
                    leaf_calls.fetch_add(1, Ordering::SeqCst);
                    Ok(Leaf(7))
                }
            }),
        ],
        scope(Request) [
            provide(|Inject(leaf): Inject<Leaf>| Ok(Mid(leaf))),
            provide(|Inject(leaf): Inject<Leaf>, Inject(mid): Inject<Mid>| Ok(Top(leaf, mid))),
        ]
    });

    let request = container.enter_build().unwrap();
    let top = request.get::<Top>().unwrap();

    assert!(RcThreadSafety::ptr_eq(&top.0, &top.1 .0));
    assert_eq!(top.0 .0, 7);

    // App-scoped Leaf is cached: depended on twice but instantiated once.
    assert_eq!(leaf_calls.load(Ordering::SeqCst), 1);

    let leaf_direct = request.get::<Leaf>().unwrap();
    assert!(RcThreadSafety::ptr_eq(&leaf_direct, &top.0));
    assert_eq!(leaf_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn test_cache_provides_false_returns_new_instance() {
    struct Uncached(u8);

    let calls = RcThreadSafety::new(AtomicU8::new(0));

    let container = Container::new(registry! {
        scope(App) [
            provide(
                {
                    let calls = calls.clone();
                    move || {
                        let prev = calls.fetch_add(1, Ordering::SeqCst);
                        Ok(Uncached(prev + 1))
                    }
                },
                config = Config { cache_provides: false },
            ),
        ]
    });

    let first = container.get::<Uncached>().unwrap();
    let second = container.get::<Uncached>().unwrap();

    assert!(!RcThreadSafety::ptr_eq(&first, &second));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(first.0, 1);
    assert_eq!(second.0, 2);
}

#[test]
fn test_default_config_caches_instance() {
    struct Cached(u8);

    let calls = RcThreadSafety::new(AtomicU8::new(0));

    let container = Container::new(registry! {
        scope(App) [
            provide({
                let calls = calls.clone();
                move || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(Cached(5))
                }
            }),
        ]
    });

    let first = container.get::<Cached>().unwrap();
    let second = container.get::<Cached>().unwrap();

    assert!(RcThreadSafety::ptr_eq(&first, &second));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(first.0, 5);
}

#[test]
fn test_instance_helper_returns_value() {
    #[derive(Clone, PartialEq, Debug)]
    struct MyClonable(u8);

    let container = Container::new(registry! {
        scope(App) [
            provide(instance(MyClonable(7))),
        ]
    });

    let got = container.get::<MyClonable>().unwrap();
    assert_eq!(*got, MyClonable(7));
    assert_eq!(got.0, 7);

    let got_transient = container.get_transient::<MyClonable>().unwrap();
    assert_eq!(got_transient, MyClonable(7));
}

#[test]
fn test_get_unregistered_returns_no_instantiator() {
    struct Registered;
    #[derive(Debug)]
    struct Unregistered;

    let container = Container::new(registry! {
        scope(App) [
            provide(|| Ok(Registered)),
        ]
    });

    let err = container.get::<Unregistered>().unwrap_err();
    assert!(matches!(err, ResolveErrorKind::NoInstantiator { .. }));

    let rendered = err.to_string();
    assert!(rendered.contains("Unregistered"), "unexpected error display: {rendered}");

    assert!(container.get::<Registered>().is_ok());
}

#[test]
fn test_inject_transient_yields_fresh_subinstances() {
    struct SubDep(u8);
    struct Outer(SubDep, SubDep);

    let sub_calls = RcThreadSafety::new(AtomicU8::new(0));

    let container = Container::new(registry! {
        scope(App) [
            provide({
                let sub_calls = sub_calls.clone();
                move || {
                    let prev = sub_calls.fetch_add(1, Ordering::SeqCst);
                    Ok(SubDep(prev + 1))
                }
            }),
        ],
        scope(Request) [
            provide(|InjectTransient(a): InjectTransient<SubDep>, InjectTransient(b): InjectTransient<SubDep>| {
                Ok(Outer(a, b))
            }),
        ]
    });

    let request = container.enter_build().unwrap();
    let outer = request.get::<Outer>().unwrap();

    // Two InjectTransient points -> two instantiator runs.
    assert_eq!(sub_calls.load(Ordering::SeqCst), 2);
    assert_eq!(outer.0 .0, 1);
    assert_eq!(outer.1 .0, 2);
    assert_ne!(outer.0 .0, outer.1 .0);

    let direct = request.get_transient::<SubDep>().unwrap();
    assert_eq!(direct.0, 3);
    assert_eq!(sub_calls.load(Ordering::SeqCst), 3);
}
