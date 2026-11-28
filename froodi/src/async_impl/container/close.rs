use alloc::boxed::Box;
use core::{
    future::{poll_fn, Future},
    pin::Pin,
    task::{ready, Context, Poll},
};
use tracing::{debug, warn};

use super::{Container, ContainerInner, InstantiatorData, SyncContainerInner};
use crate::{
    async_impl::{finalizer::BoxedCloneFinalizer, service::Service as _},
    cache::{Resolved, ResolvedSet},
    utils::{future::BoxFuture, thread_safety::SendSafety},
};

impl Container {
    /// Closes the container, calling finalizers for resolved dependencies in LIFO order.
    ///
    /// # Warning
    /// This method can be called multiple times, but it will only call finalizers for dependencies that were resolved since the last call
    pub fn close(&self) -> impl Future<Output = ()> + SendSafety + '_ {
        let mut suspended = None;
        poll_fn(move |cx| match &mut suspended {
            None => {
                let mut closing = Closing::new(self);
                let result = Pin::new(&mut closing).poll(cx);
                if result.is_pending() {
                    suspended = Some(Box::pin(closing));
                }
                result
            }
            Some(closing) => closing.as_mut().poll(cx),
        })
    }
}

struct Closing<'a> {
    async_close: AsyncClosing<'a>,
    sync_scope: &'a SyncContainerInner,
    close_initial_sync: bool,
}

impl<'a> Closing<'a> {
    fn new(container: &'a Container) -> Self {
        Self {
            async_close: AsyncClosing::new(container.inner.as_ref()),
            sync_scope: container.sync.inner.as_ref(),
            close_initial_sync: true,
        }
    }
}

impl Future for Closing<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        // The initial scope closes async first. Each parent closes sync first.
        ready!(Pin::new(&mut this.async_close).poll(cx));
        if this.close_initial_sync {
            this.sync_scope.close_scope();
            this.close_initial_sync = false;
        }

        while this.async_close.scope.close_parent {
            let (parent, sync_parent) = match (this.async_close.scope.parent.as_deref(), this.sync_scope.parent.as_ref()) {
                (Some(parent), Some(sync_parent)) => (parent, sync_parent.inner.as_ref()),
                (None, None) => break,
                _ => unreachable!(),
            };

            sync_parent.close_scope();
            this.async_close = AsyncClosing::new(parent);
            this.sync_scope = sync_parent;
            ready!(Pin::new(&mut this.async_close).poll(cx));
        }

        Poll::Ready(())
    }
}

struct RunningFinalizer {
    future: BoxFuture<'static, Result<(), ()>>,
    // Keep the cloned service alive until its call completes or is cancelled.
    _service: BoxedCloneFinalizer,
    type_name: &'static str,
}

struct AsyncClosing<'a> {
    running: Option<RunningFinalizer>,
    remaining: Option<ResolvedSet>,
    scope: &'a ContainerInner,
}

impl<'a> AsyncClosing<'a> {
    fn new(scope: &'a ContainerInner) -> Self {
        let mut cache = scope.cache.write();
        let remaining = if cache.resolved.0.is_empty() {
            cache.map.clone_from(&scope.context.map);
            None
        } else {
            Some(cache.take_resolved_set())
        };
        Self {
            running: None,
            remaining,
            scope,
        }
    }
}

impl Future for AsyncClosing<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let Some(remaining) = &mut this.remaining else {
            return Poll::Ready(());
        };
        loop {
            if let Some(running) = &mut this.running {
                let _ = ready!(running.future.as_mut().poll(cx));
                let type_name = running.type_name;
                this.running = None;
                debug!(type_info = %type_name, "Finalizer called");
            }

            let Some(Resolved { type_info, dependency }) = remaining.0.pop_back() else {
                break;
            };
            let InstantiatorData { finalizer, .. } = this
                .scope
                .registry
                .get_async(&type_info)
                .expect("Instantiator should be present for resolved type");
            if let Some(finalizer) = finalizer {
                let mut service = finalizer.clone();
                this.running = Some(RunningFinalizer {
                    future: service.call(dependency),
                    _service: service,
                    type_name: type_info.name,
                });
            }
        }
        this.remaining = None;
        this.scope.cache.write().map.clone_from(&this.scope.context.map);
        Poll::Ready(())
    }
}

impl Drop for ContainerInner {
    fn drop(&mut self) {
        let pending = self.cache.get_mut().resolved.0.len();
        if pending > 0 {
            warn!(
                scope = %self.scope_data,
                pending,
                "async Container dropped without close().await; {pending} finalizer(s) will not run",
            );
        }
    }
}
