#![cfg(feature = "async")]

use froodi::{async_impl::Container, registry, DefaultScope::App};
use std::{
    future::{pending, Future},
    marker::PhantomPinned,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll, Wake, Waker},
};

fn assert_unpin(_: &(impl Future + Unpin)) {}
#[cfg(feature = "thread_safe")]
fn assert_send(_: &(impl Future + Send)) {}

#[test]
fn resolution_futures_keep_unpin_even_for_non_unpin_values() {
    let container = Container::new(registry! { provide(App, async || Ok(PhantomPinned)) });
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

#[tokio::test]
async fn cached_resolution_observes_cache_when_polled_not_when_created() {
    let calls = Arc::new(AtomicUsize::new(0));
    let factory_calls = calls.clone();
    let container = Container::new(registry! { provide(App, move || {
        let calls = factory_calls.clone();
        async move { Ok(calls.fetch_add(1, Ordering::SeqCst)) }
    }) });
    assert_eq!(*container.get::<usize>().await.unwrap(), 0);
    let future = container.get::<usize>();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    container.close().await;
    assert_eq!(*future.await.unwrap(), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

struct NoopWake;
impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

#[test]
fn cancelling_cold_resolution_releases_the_instantiation_lock() {
    let calls = Arc::new(AtomicUsize::new(0));
    let factory_calls = calls.clone();
    let container = Container::new(registry! { provide(App, move || {
        let calls = factory_calls.clone();
        async move {
            if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                pending::<()>().await;
            }
            Ok(42_u32)
        }
    }) });
    let waker = Waker::from(Arc::new(NoopWake));
    let mut cx = Context::from_waker(&waker);
    let mut first = Box::pin(container.get::<u32>());
    assert!(first.as_mut().poll(&mut cx).is_pending());
    drop(first);
    let mut retry = Box::pin(container.get::<u32>());
    match retry.as_mut().poll(&mut cx) {
        Poll::Ready(Ok(value)) => assert_eq!(*value, 42),
        _ => panic!("retry should acquire the released lock and finish immediately"),
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
