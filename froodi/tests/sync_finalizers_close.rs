#![no_std]

extern crate alloc;

use core::sync::atomic::{AtomicU8, Ordering};
use froodi::utils::thread_safety::RcThreadSafety;
use froodi::{
    registry, Config, Container,
    DefaultScope::{App, Request},
    InstantiateErrorKind,
};

struct Dep1;
struct Dep2;
struct NeverResolved;
struct Reset;
struct NoCacheFinalized;
struct ParentDep;
struct ChildDep;

/// LIFO: dep resolved last (Dep2) is finalized first.
#[test]
fn test_finalizers_run_lifo_on_close() {
    let tick = RcThreadSafety::new(AtomicU8::new(0));

    let dep1_fin_count = RcThreadSafety::new(AtomicU8::new(0));
    let dep1_fin_pos = RcThreadSafety::new(AtomicU8::new(0));
    let dep2_fin_count = RcThreadSafety::new(AtomicU8::new(0));
    let dep2_fin_pos = RcThreadSafety::new(AtomicU8::new(0));

    let container = Container::new(registry! {
        scope(Request) [
            provide(
                || Ok(Dep1),
                finalizer = {
                    let tick = tick.clone();
                    let dep1_fin_count = dep1_fin_count.clone();
                    let dep1_fin_pos = dep1_fin_pos.clone();
                    move |_: RcThreadSafety<Dep1>| {
                        let pos = tick.fetch_add(1, Ordering::SeqCst) + 1;
                        dep1_fin_pos.store(pos, Ordering::SeqCst);
                        dep1_fin_count.fetch_add(1, Ordering::SeqCst);
                    }
                }
            ),
            provide(
                || Ok(Dep2),
                finalizer = {
                    let tick = tick.clone();
                    let dep2_fin_count = dep2_fin_count.clone();
                    let dep2_fin_pos = dep2_fin_pos.clone();
                    move |_: RcThreadSafety<Dep2>| {
                        let pos = tick.fetch_add(1, Ordering::SeqCst) + 1;
                        dep2_fin_pos.store(pos, Ordering::SeqCst);
                        dep2_fin_count.fetch_add(1, Ordering::SeqCst);
                    }
                }
            ),
        ],
    });
    let request = container.enter().with_scope(Request).build().unwrap();

    let _ = request.get::<Dep1>().unwrap();
    let _ = request.get::<Dep2>().unwrap();

    request.close();

    assert_eq!(dep1_fin_count.load(Ordering::SeqCst), 1, "Dep1 finalizer ran once");
    assert_eq!(dep2_fin_count.load(Ordering::SeqCst), 1, "Dep2 finalizer ran once");
    assert_eq!(dep2_fin_pos.load(Ordering::SeqCst), 1, "Dep2 finalized first (LIFO)");
    assert_eq!(dep1_fin_pos.load(Ordering::SeqCst), 2, "Dep1 finalized second (LIFO)");
    assert!(
        dep2_fin_pos.load(Ordering::SeqCst) < dep1_fin_pos.load(Ordering::SeqCst),
        "Dep2 finalizer ran strictly before Dep1 finalizer",
    );
}

#[test]
fn test_finalizer_not_called_for_unresolved() {
    let fin_count = RcThreadSafety::new(AtomicU8::new(0));

    let container = Container::new(registry! {
        scope(Request) [
            provide(
                || Ok(NeverResolved),
                finalizer = {
                    let fin_count = fin_count.clone();
                    move |_: RcThreadSafety<NeverResolved>| {
                        fin_count.fetch_add(1, Ordering::SeqCst);
                    }
                }
            ),
        ],
    });
    let request = container.enter().with_scope(Request).build().unwrap();

    request.close();

    assert_eq!(
        fin_count.load(Ordering::SeqCst),
        0,
        "finalizer must not run for an unresolved dependency",
    );
}

#[test]
fn test_close_resets_cache_fresh_instance() {
    let inst_count = RcThreadSafety::new(AtomicU8::new(0));

    let container = Container::new(registry! {
        scope(Request) [
            provide({
                let inst_count = inst_count.clone();
                move || {
                    inst_count.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, InstantiateErrorKind>(Reset)
                }
            }),
        ],
    });
    let request = container.enter().with_scope(Request).build().unwrap();

    let first = request.get::<Reset>().unwrap();
    assert_eq!(inst_count.load(Ordering::SeqCst), 1, "instantiator ran once before close");

    let first_again = request.get::<Reset>().unwrap();
    assert_eq!(inst_count.load(Ordering::SeqCst), 1, "cached get does not re-run instantiator");
    assert!(RcThreadSafety::ptr_eq(&first, &first_again), "cached get returns same Arc");

    request.close();

    let second = request.get::<Reset>().unwrap();
    assert_eq!(
        inst_count.load(Ordering::SeqCst),
        2,
        "instantiator re-ran after close cleared cache"
    );
    assert!(
        !RcThreadSafety::ptr_eq(&first, &second),
        "post-close get returns a fresh instance, not the pre-close one",
    );
}

#[test]
fn test_close_is_idempotent() {
    let fin_count = RcThreadSafety::new(AtomicU8::new(0));

    let container = Container::new(registry! {
        scope(Request) [
            provide(
                || Ok(Dep1),
                finalizer = {
                    let fin_count = fin_count.clone();
                    move |_: RcThreadSafety<Dep1>| {
                        fin_count.fetch_add(1, Ordering::SeqCst);
                    }
                }
            ),
        ],
    });
    let request = container.enter().with_scope(Request).build().unwrap();

    let _ = request.get::<Dep1>().unwrap();

    request.close();
    assert_eq!(fin_count.load(Ordering::SeqCst), 1, "first close ran the finalizer once");

    request.close();
    assert_eq!(fin_count.load(Ordering::SeqCst), 1, "second close runs no finalizers (idempotent)",);

    request.close();
    assert_eq!(fin_count.load(Ordering::SeqCst), 1, "repeated close stays idempotent");
}

/// Finalizer forces resolved-set push even when cache_provides=false.
#[test]
fn test_finalizer_runs_with_cache_provides_false() {
    let fin_count = RcThreadSafety::new(AtomicU8::new(0));
    let inst_count = RcThreadSafety::new(AtomicU8::new(0));

    let container = Container::new(registry! {
        scope(Request) [
            provide(
                {
                    let inst_count = inst_count.clone();
                    move || {
                        inst_count.fetch_add(1, Ordering::SeqCst);
                        Ok::<_, InstantiateErrorKind>(NoCacheFinalized)
                    }
                },
                config = Config { cache_provides: false },
                finalizer = {
                    let fin_count = fin_count.clone();
                    move |_: RcThreadSafety<NoCacheFinalized>| {
                        fin_count.fetch_add(1, Ordering::SeqCst);
                    }
                }
            ),
        ],
    });
    let request = container.enter().with_scope(Request).build().unwrap();

    let _ = request.get::<NoCacheFinalized>().unwrap();
    assert_eq!(inst_count.load(Ordering::SeqCst), 1, "instantiator ran once");

    request.close();

    assert_eq!(
        fin_count.load(Ordering::SeqCst),
        1,
        "finalizer runs even with cache_provides=false (resolved-set push forced by finalizer)",
    );
}

/// App-scoped deps are finalized by their owning App container, not by a closing child.
#[test]
fn test_close_child_does_not_finalize_parent() {
    let parent_fin_count = RcThreadSafety::new(AtomicU8::new(0));
    let child_fin_count = RcThreadSafety::new(AtomicU8::new(0));

    let app = Container::new(registry! {
        scope(App) [
            provide(
                || Ok(ParentDep),
                finalizer = {
                    let parent_fin_count = parent_fin_count.clone();
                    move |_: RcThreadSafety<ParentDep>| {
                        parent_fin_count.fetch_add(1, Ordering::SeqCst);
                    }
                }
            ),
        ],
        scope(Request) [
            provide(
                || Ok(ChildDep),
                finalizer = {
                    let child_fin_count = child_fin_count.clone();
                    move |_: RcThreadSafety<ChildDep>| {
                        child_fin_count.fetch_add(1, Ordering::SeqCst);
                    }
                }
            ),
        ],
    });
    let request = app.clone().enter().with_scope(Request).build().unwrap();

    let _ = request.get::<ParentDep>().unwrap();
    let _ = request.get::<ChildDep>().unwrap();

    request.close();

    assert_eq!(
        child_fin_count.load(Ordering::SeqCst),
        1,
        "closing child finalizes the child-scoped dep",
    );
    assert_eq!(
        parent_fin_count.load(Ordering::SeqCst),
        0,
        "closing child does NOT finalize the parent-scoped dep",
    );

    app.close();
    assert_eq!(
        parent_fin_count.load(Ordering::SeqCst),
        1,
        "closing the parent (App) finalizes the parent-scoped dep",
    );
    assert_eq!(
        child_fin_count.load(Ordering::SeqCst),
        1,
        "child finalizer not run again by parent close",
    );
}
