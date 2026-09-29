#[cfg(feature = "thread_safe")]
use froodi_compile::{registry, Container};
#[cfg(any(feature = "thread_safe", feature = "async"))]
use froodi_compile::{thread_safety::RcThreadSafety, DefaultScope::App, InstantiateErrorKind};

#[cfg(feature = "thread_safe")]
#[test]
fn concurrent_cached_construction() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Barrier,
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let inst = {
        let (calls, entered, release) = (calls.clone(), entered.clone(), release.clone());
        move || {
            if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                entered.wait();
                release.wait();
            }
            Ok::<_, InstantiateErrorKind>(String::from("value"))
        }
    };
    let container = Container::new(registry! { provide(App, inst) });
    std::thread::scope(|scope| {
        let first = scope.spawn(|| container.get::<String>().unwrap());
        entered.wait();
        let start = Arc::new(Barrier::new(8));
        let others: Vec<_> = (0..7)
            .map(|_| {
                let start = start.clone();
                let container = &container;
                scope.spawn(move || {
                    start.wait();
                    container.get::<String>().unwrap()
                })
            })
            .collect();
        start.wait();
        release.wait();
        let first = first.join().unwrap();
        for other in others {
            assert!(RcThreadSafety::ptr_eq(&first, &other.join().unwrap()));
        }
    });
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[cfg(feature = "async")]
#[tokio::test(flavor = "current_thread")]
async fn overlapping_async_requests_are_serialized_even_without_threads() {
    use froodi_compile::{async_impl::Container, async_registry, Config};
    use std::{
        future::Future,
        sync::atomic::{AtomicUsize, Ordering},
        task::{Context, Poll, Waker},
    };
    let calls = RcThreadSafety::new(AtomicUsize::new(0));
    let gate = RcThreadSafety::new(tokio::sync::Notify::new());
    let inst = {
        let (calls, gate) = (calls.clone(), gate.clone());
        move || {
            let (calls, gate) = (calls.clone(), gate.clone());
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                gate.notified().await;
                Ok::<_, InstantiateErrorKind>(String::from("value"))
            }
        }
    };
    let container = Container::new(async_registry! { provide(App, inst) });
    let mut first = Box::pin(container.get::<String>());
    let mut second = Box::pin(container.get::<String>());
    let mut cx = Context::from_waker(Waker::noop());
    assert!(matches!(first.as_mut().poll(&mut cx), Poll::Pending));
    assert!(matches!(second.as_mut().poll(&mut cx), Poll::Pending));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    gate.notify_one();
    let first = first.await.unwrap();
    let second = second.await.unwrap();
    assert!(RcThreadSafety::ptr_eq(&first, &second));
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let count = calls.clone();
    let uncached = Container::new(async_registry! {
        provide(App, move || { let count = count.clone(); async move {
            Ok::<_, InstantiateErrorKind>(count.fetch_add(1, Ordering::SeqCst))
        } }, config = Config { cache_provides: false }),
    });
    let one = uncached.get::<usize>().await.unwrap();
    let two = uncached.get::<usize>().await.unwrap();
    assert!(!RcThreadSafety::ptr_eq(&one, &two));
    assert_ne!(*one, *two);
    assert_ne!(
        uncached.get_transient::<usize>().await.unwrap(),
        uncached.get_transient::<usize>().await.unwrap()
    );
}

#[cfg(all(feature = "async", feature = "thread_safe"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cached_async_requests_from_multiple_tasks_share_one_allocation() {
    use froodi_compile::{async_impl::Container, async_registry};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let inst = {
        let (calls, entered, release) = (calls.clone(), entered.clone(), release.clone());
        move || {
            let (calls, entered, release) = (calls.clone(), entered.clone(), release.clone());
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                entered.notify_one();
                release.acquire().await.unwrap().forget();
                Ok::<_, InstantiateErrorKind>(String::from("value"))
            }
        }
    };
    let container = Container::new(async_registry! { provide(App, inst) });
    let first = {
        let container = container.clone();
        tokio::spawn(async move { container.get::<String>().await.unwrap() })
    };
    entered.notified().await;
    let start = Arc::new(tokio::sync::Barrier::new(8));
    let others: Vec<_> = (0..7)
        .map(|_| {
            let (start, container) = (start.clone(), container.clone());
            tokio::spawn(async move {
                start.wait().await;
                container.get::<String>().await.unwrap()
            })
        })
        .collect();
    start.wait().await;
    release.add_permits(8);
    let first = first.await.unwrap();
    for other in others {
        assert!(Arc::ptr_eq(&first, &other.await.unwrap()));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
