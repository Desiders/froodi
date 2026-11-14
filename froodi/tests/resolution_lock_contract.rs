//! Resolution lock ownership, serialization, and future contracts.
use froodi::{registry, utils::thread_safety::RcThreadSafety, Container, DefaultScope::App};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Debug)]
struct Payload(usize);

#[cfg(feature = "thread_safe")]
#[test]
fn sync_concurrent_cached_get_constructs_one_shared_value() {
    use std::{sync::Barrier, thread, time::Duration};
    let calls = Arc::new(AtomicUsize::new(0));
    let factory_calls = calls.clone();
    let container = Container::new(registry! {
        provide(App, move || {
            let id = factory_calls.fetch_add(1, Ordering::SeqCst);
            thread::sleep(Duration::from_millis(2));
            Ok(Payload(id))
        }),
    });
    let start = Arc::new(Barrier::new(8));
    let values = thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let container = container.clone();
                let start = start.clone();
                scope.spawn(move || {
                    start.wait();
                    container.get::<Payload>().unwrap()
                })
            })
            .collect();
        handles.into_iter().map(|handle| handle.join().unwrap()).collect::<Vec<_>>()
    });
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(values[0].0, 0);
    assert!(values.iter().all(|value| RcThreadSafety::ptr_eq(value, &values[0])));
}

#[cfg(feature = "async")]
mod asynchronous {
    use super::*;
    use froodi::{
        async_impl::{Container as AsyncContainer, Instantiator},
        Config,
        DefaultScope::Request,
        Dependency, InstantiateErrorKind,
    };
    use std::{
        collections::BTreeSet,
        future::Future,
        marker::PhantomPinned,
        sync::Mutex,
        task::{Context, Poll, Wake, Waker},
    };
    use tokio::sync::Semaphore;

    fn assert_unpin(_: &(impl Future + Unpin)) {}
    #[cfg(feature = "thread_safe")]
    fn assert_send(_: &(impl Future + Send)) {}

    #[test]
    fn resolution_futures_keep_unpin_and_send_with_non_unpin_output() {
        let container = AsyncContainer::new(registry! { provide(App, async || Ok(PhantomPinned)) });
        let cached = container.get::<PhantomPinned>();
        let transient = container.get_transient::<PhantomPinned>();
        assert_unpin(&cached);
        assert_unpin(&transient);
        #[cfg(feature = "thread_safe")]
        {
            assert_send(&cached);
            assert_send(&transient);
        }
    }

    struct CountingFactory {
        clones: Arc<AtomicUsize>,
        calls: Arc<AtomicUsize>,
    }
    impl Clone for CountingFactory {
        fn clone(&self) -> Self {
            self.clones.fetch_add(1, Ordering::SeqCst);
            Self {
                clones: self.clones.clone(),
                calls: self.calls.clone(),
            }
        }
    }
    impl Instantiator<()> for CountingFactory {
        type Provides = usize;
        type Error = InstantiateErrorKind;
        async fn instantiate(&mut self, _: ()) -> Result<Self::Provides, Self::Error> {
            Ok(self.calls.fetch_add(1, Ordering::SeqCst))
        }
        fn dependencies() -> BTreeSet<Dependency> {
            BTreeSet::new()
        }
    }

    #[tokio::test]
    async fn future_construction_is_lazy_and_factory_clone_policy_stays_the_same() {
        let clones = Arc::new(AtomicUsize::new(0));
        let calls = Arc::new(AtomicUsize::new(0));
        let container = AsyncContainer::new(registry! {
            provide(App, CountingFactory { clones: clones.clone(), calls: calls.clone() }),
        });
        let initial_clones = clones.load(Ordering::SeqCst);
        let never_polled = container.get::<usize>();
        assert_eq!(clones.load(Ordering::SeqCst), initial_clones);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        drop(never_polled);
        assert_eq!(clones.load(Ordering::SeqCst), initial_clones);
        let first = container.get::<usize>().await.unwrap();
        assert_eq!(*first, 0);
        assert_eq!(clones.load(Ordering::SeqCst), initial_clones + 1);
        let cached = container.get::<usize>().await.unwrap();
        assert!(RcThreadSafety::ptr_eq(&first, &cached));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(clones.load(Ordering::SeqCst), initial_clones + 1);
        let delayed = container.get::<usize>();
        container.close().await;
        assert_eq!(*delayed.await.unwrap(), 1, "lookup must observe cache at first poll");
        assert_eq!(container.get_transient::<usize>().await.unwrap(), 2);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        assert_eq!(clones.load(Ordering::SeqCst), initial_clones + 3);
    }

    #[cfg(feature = "thread_safe")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn async_concurrent_cached_get_constructs_one_shared_value() {
        use tokio::{
            spawn,
            sync::{Barrier, Notify},
            task::yield_now,
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let permits = Arc::new(Semaphore::new(0));
        let entered = Arc::new(Notify::new());
        let (factory_calls, factory_permits, factory_entered) = (calls.clone(), permits.clone(), entered.clone());
        let container = AsyncContainer::new(registry! {
            provide(App, move || {
                let (calls, permits, entered) = (factory_calls.clone(), factory_permits.clone(), factory_entered.clone());
                async move {
                    let id = calls.fetch_add(1, Ordering::SeqCst);
                    entered.notify_one();
                    permits.acquire().await.unwrap().forget();
                    Ok(Payload(id))
                }
            }),
        });
        let start = Arc::new(Barrier::new(9));
        let jobs: Vec<_> = (0..8)
            .map(|_| {
                let container = container.clone();
                let start = start.clone();
                spawn(async move {
                    start.wait().await;
                    container.get::<Payload>().await.unwrap()
                })
            })
            .collect();
        start.wait().await;
        entered.notified().await;
        for _ in 0..4 {
            yield_now().await;
        }
        permits.add_permits(8);
        let mut values = Vec::new();
        for job in jobs {
            values.push(job.await.unwrap());
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(values[0].0, 0);
        assert!(values.iter().all(|value| RcThreadSafety::ptr_eq(value, &values[0])));
    }

    struct NoopWake;
    impl Wake for NoopWake {
        fn wake(self: Arc<Self>) {}
    }

    #[tokio::test]
    async fn uncached_get_keeps_same_type_serialization_and_returns_fresh_values() {
        let calls = Arc::new(AtomicUsize::new(0));
        let permits = Arc::new(Semaphore::new(0));
        let (factory_calls, factory_permits) = (calls.clone(), permits.clone());
        let container = AsyncContainer::new(registry! {
            provide(App, move || {
                let (calls, permits) = (factory_calls.clone(), factory_permits.clone());
                async move {
                    let id = calls.fetch_add(1, Ordering::SeqCst);
                    permits.acquire().await.unwrap().forget();
                    Ok(Payload(id))
                }
            }, config = Config { cache_provides: false }),
        });
        let waker = Waker::from(Arc::new(NoopWake));
        let mut cx = Context::from_waker(&waker);
        let mut first = Box::pin(container.get::<Payload>());
        let mut second = Box::pin(container.get::<Payload>());
        assert!(first.as_mut().poll(&mut cx).is_pending());
        assert!(second.as_mut().poll(&mut cx).is_pending());
        assert_eq!(calls.load(Ordering::SeqCst), 1, "second factory must wait for the same type lock");
        permits.add_permits(1);
        let first_value = first.await.unwrap();
        assert!(second.as_mut().poll(&mut cx).is_pending());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        permits.add_permits(1);
        let second_value = second.await.unwrap();
        assert_eq!((first_value.0, second_value.0), (0, 1));
        assert!(!RcThreadSafety::ptr_eq(&first_value, &second_value));
    }

    struct Tracked {
        id: usize,
        drops: Arc<AtomicUsize>,
    }
    impl Drop for Tracked {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn cancellation_releases_waiters_and_only_completed_values_get_finalized() {
        let calls = Arc::new(AtomicUsize::new(0));
        let drops = Arc::new(AtomicUsize::new(0));
        let permits = Arc::new(Semaphore::new(0));
        let finalized = Arc::new(Mutex::new(Vec::new()));
        let (factory_calls, factory_drops, factory_permits, finalizer_log) =
            (calls.clone(), drops.clone(), permits.clone(), finalized.clone());
        let app = AsyncContainer::new(registry! {
            provide(Request, move || {
                let (calls, drops, permits) = (factory_calls.clone(), factory_drops.clone(), factory_permits.clone());
                async move {
                    let value = Tracked { id: calls.fetch_add(1, Ordering::SeqCst), drops };
                    permits.acquire().await.unwrap().forget();
                    Ok(value)
                }
            }, finalizer = move |value: RcThreadSafety<Tracked>| {
                let log = finalizer_log.clone();
                async move { log.lock().unwrap().push(value.id); }
            }),
        });
        let request = app.clone().enter_build().unwrap();
        let waker = Waker::from(Arc::new(NoopWake));
        let mut cx = Context::from_waker(&waker);
        let mut first = Box::pin(request.get::<Tracked>());
        let mut waiter = Box::pin(request.get::<Tracked>());
        assert!(first.as_mut().poll(&mut cx).is_pending());
        assert!(waiter.as_mut().poll(&mut cx).is_pending());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        drop(waiter);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(first);
        assert_eq!(drops.load(Ordering::SeqCst), 1, "cancelled factory owns its unfinished value");
        assert!(finalized.lock().unwrap().is_empty());
        permits.add_permits(1);
        let mut retry = Box::pin(request.get::<Tracked>());
        let value = match retry.as_mut().poll(&mut cx) {
            Poll::Ready(Ok(value)) => value,
            _ => panic!("cancelled lookup must release its instantiation lock"),
        };
        drop(retry);
        assert_eq!(value.id, 1);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        request.close().await;
        assert_eq!(*finalized.lock().unwrap(), [1]);
        assert_eq!(drops.load(Ordering::SeqCst), 1, "returned handle still owns the completed value");
        drop(value);
        assert_eq!(drops.load(Ordering::SeqCst), 2);
        app.close().await;
        assert_eq!(*finalized.lock().unwrap(), [1]);
    }
}
