#![cfg(feature = "async")]

use froodi::{async_impl::Container, registry, DefaultScope::App};
use std::{
    future::{pending, poll_fn, Future},
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
fn resolution_futures_keep_unpin() {
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
async fn cached_resolution_reads_cache_on_first_poll() {
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
fn cancelling_get_releases_instantiation_lock() {
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

#[test]
fn transient_resolution_resumes_parent_factory_after_pending() {
    let container = Container::new(registry! { provide(App, async || {
        let mut ready = false;
        poll_fn(move |cx| {
            if ready {
                return Poll::Ready(Ok(42_u64));
            }
            ready = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }).await
    }) });
    let child = container.enter_build().unwrap();
    let mut future = Box::pin(child.get_transient::<u64>());
    let waker = Waker::from(Arc::new(NoopWake));
    let mut cx = Context::from_waker(&waker);

    assert!(future.as_mut().poll(&mut cx).is_pending());
    assert!(matches!(future.as_mut().poll(&mut cx), Poll::Ready(Ok(42))));
}
