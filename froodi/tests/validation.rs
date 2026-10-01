#![cfg(feature = "compiled")]

use froodi::compiled_registry as registry;
use froodi::{instance, Container, DefaultScope::App, Inject, InjectTransient, InstantiateErrorKind};

#[cfg(not(miri))]
include!("fixtures/padding.rs");

#[derive(Clone)]
struct Base(usize);

type Alias = Base;

struct Left(usize);
struct Right(usize);
struct End(usize);

fn left(dep: Inject<Alias>) -> Result<Left, InstantiateErrorKind> {
    Ok(Left(dep.0 .0))
}

fn right(dep: InjectTransient<Base>) -> Result<Right, InstantiateErrorKind> {
    Ok(Right(dep.0 .0))
}

#[test]
fn links_fragments_with_forward_references_and_mixed_injection() {
    let captured = 3;
    // Neither fragment is linked until final composition; declaration order is deliberately reversed.
    let fragment = registry! { provide(App, left), provide(App, right) };
    let container = Container::new(registry! {
        provide(App, move |left: Inject<Left>, right: Inject<Right>| Ok::<_, InstantiateErrorKind>(End(left.0.0 + right.0.0 + captured))),
        extend(fragment),
        provide(App, instance(Base(7))),
    });

    assert_eq!(container.get::<End>().unwrap().0, 17);
}

#[cfg(not(miri))]
#[test]
fn rejects_oversized_sync_cycles_at_runtime() {
    struct Cycle;

    on_large_stack(|| {
        // 1,022 padding leaves + Cycle + extra unit leaf + implicit container = 1,025.
        let result = std::panic::catch_unwind(|| {
            let _ = Container::new(registry! {
                provide(App, |_: Inject<Cycle>| Ok::<_, InstantiateErrorKind>(Cycle)),
                provide(App, || Ok::<_, InstantiateErrorKind>(())),
                extend(padding_1022!()),
            });
        });

        assert!(result.is_err());
    });
}

#[cfg(all(feature = "async", not(miri)))]
#[test]
fn rejects_oversized_async_cycles_at_runtime() {
    use froodi::{async_impl::Container, compiled_async_registry as registry};

    struct Cycle;

    on_large_stack(|| {
        // 1,021 padding leaves + Cycle + extra unit leaf + two implicit containers = 1,025.
        let result = std::panic::catch_unwind(|| {
            let _ = Container::new(registry! {
                provide(App, async |_: Inject<Cycle>| Ok::<_, InstantiateErrorKind>(Cycle)),
                provide(App, async || Ok::<_, InstantiateErrorKind>(())),
                extend(padding_1021!()),
            });
        });

        assert!(result.is_err());
    });
}

#[cfg(not(miri))]
fn on_large_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}
