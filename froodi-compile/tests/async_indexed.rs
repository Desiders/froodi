//! Indexed async edges keep trait obligations shallow and preserve runtime dispatch semantics.
#![cfg(feature = "async")]

use froodi_compile::{
    async_impl::Container, async_registry, registry, Config, DefaultScope::*, Inject, InjectTransient, InstantiateErrorKind,
};

struct Value<const N: usize>(usize);

async fn next<const N: usize, const PREV: usize>(Inject(previous): Inject<Value<PREV>>) -> Result<Value<N>, InstantiateErrorKind> {
    Ok(Value(previous.0 + 1))
}

#[tokio::test]
async fn resolves_a_deep_async_chain_under_the_default_recursion_limit() {
    macro_rules! chain {
        ($($n:literal => $prev:literal),*) => {
            Container::new(async_registry! {
                scope(App) [
                    provide(async || Ok::<_, InstantiateErrorKind>(Value::<0>(0))),
                    $(provide(next::<$n, $prev>),)*
                ],
            })
        };
    }
    let container = chain!(
        1 => 0, 2 => 1, 3 => 2, 4 => 3, 5 => 4, 6 => 5, 7 => 6, 8 => 7, 9 => 8, 10 => 9,
        11 => 10, 12 => 11, 13 => 12, 14 => 13, 15 => 14, 16 => 15, 17 => 16, 18 => 17, 19 => 18, 20 => 19,
        21 => 20, 22 => 21, 23 => 22, 24 => 23, 25 => 24, 26 => 25, 27 => 26, 28 => 27, 29 => 28, 30 => 29,
        31 => 30, 32 => 31, 33 => 32, 34 => 33, 35 => 34, 36 => 35, 37 => 36, 38 => 37, 39 => 38, 40 => 39,
        41 => 40, 42 => 41, 43 => 42, 44 => 43, 45 => 44, 46 => 45, 47 => 46, 48 => 47, 49 => 48, 50 => 49,
        51 => 50, 52 => 51, 53 => 52, 54 => 53, 55 => 54, 56 => 55, 57 => 56, 58 => 57, 59 => 58, 60 => 59,
        61 => 60, 62 => 61, 63 => 62, 64 => 63, 65 => 64, 66 => 65, 67 => 66, 68 => 67, 69 => 68, 70 => 69,
        71 => 70, 72 => 71, 73 => 72, 74 => 73, 75 => 74, 76 => 75, 77 => 76, 78 => 77, 79 => 78, 80 => 79,
        81 => 80, 82 => 81, 83 => 82, 84 => 83, 85 => 84, 86 => 85, 87 => 86, 88 => 87, 89 => 88, 90 => 89,
        91 => 90, 92 => 91, 93 => 92, 94 => 93, 95 => 94, 96 => 95, 97 => 96, 98 => 97, 99 => 98
    );
    assert_eq!(container.get::<Value<99>>().await.unwrap().0, 99);
}

#[test]
fn async_cycles_reach_the_graph_compiler() {
    struct First;
    struct Second;
    let result = Container::try_new(async_registry! {
        scope(App) [
            provide(async |_: Inject<Second>| Ok::<_, InstantiateErrorKind>(First)),
            provide(async |_: InjectTransient<First>| Ok::<_, InstantiateErrorKind>(Second)),
        ],
    });
    let diagnostic = result.err().expect("cycle must be diagnosed").to_string();
    assert!(diagnostic.contains("First"));
    assert!(diagnostic.contains("Second"));
    assert!(diagnostic.to_lowercase().contains("cycle"));
}

#[tokio::test]
async fn async_dependencies_follow_runtime_replacements_and_cache_policy() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };

    struct Token(usize);
    struct Pair(usize, usize);
    let calls = Arc::new(AtomicUsize::new(0));
    let finalized = Arc::new(Mutex::new(Vec::new()));
    let count = calls.clone();
    let log = finalized.clone();
    let replacement = registry! {
        provide(App, move || Ok::<_, InstantiateErrorKind>(Token(count.fetch_add(1, Ordering::SeqCst))),
            config = Config { cache_provides: false },
            finalizer = move |token: froodi_compile::thread_safety::RcThreadSafety<Token>| log.lock().unwrap().push(token.0)),
    }
    .into_runtime()
    .replacing();
    let container = Container::new(async_registry! {
        scope(App) [
            provide(async || Ok::<_, InstantiateErrorKind>(Token(999))),
            provide(async |Inject(first): Inject<Token>, InjectTransient(second): InjectTransient<Token>|
                Ok::<_, InstantiateErrorKind>(Pair(first.0, second.0)),
                config = Config { cache_provides: false }),
        ],
        extend(replacement),
    });
    let first = container.get::<Pair>().await.unwrap();
    let second = container.get::<Pair>().await.unwrap();
    assert_eq!((first.0, first.1, second.0, second.1), (0, 1, 2, 3));
    container.close().await;
    // InjectTransient values are owned by the consumer and do not enter the finalizer list.
    assert_eq!(*finalized.lock().unwrap(), [2, 0]);
}

#[tokio::test]
async fn async_transient_dependencies_construct_in_their_owning_scope() {
    struct RequestOnly;
    struct OwnerCheck(bool);
    struct Consumer(bool);
    let app = Container::new(async_registry! {
        provide(App, async |Inject(owner): Inject<froodi_compile::Container>|
            Ok::<_, InstantiateErrorKind>(OwnerCheck(owner.get::<RequestOnly>().is_err()))),
        provide(Request, async |InjectTransient(check): InjectTransient<OwnerCheck>|
            Ok::<_, InstantiateErrorKind>(Consumer(check.0))),
        extend(registry! { provide(Request, || Ok::<_, InstantiateErrorKind>(RequestOnly)) }),
    });
    let request = app.enter_build().unwrap();
    assert!(request.get::<Consumer>().await.unwrap().0);
}
