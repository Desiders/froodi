use alloc::boxed::Box;
use core::{
    future::{poll_fn, Future},
    task::{ready, Context, Poll},
};
use tracing::debug;

use super::{Container, ContainerInner, SyncContainerInner};
use crate::{
    async_impl::{finalizer::BoxedCloneFinalizer, service::Service as _},
    cache::{Resolved, ResolvedSet},
    utils::{future::BoxFuture, thread_safety::SendSafety},
    TypeInfo,
};

impl Container {
    /// Closes the container, calling finalizers for resolved dependencies in LIFO order.
    ///
    /// # Warning
    /// This method can be called multiple times, but it will only call finalizers for dependencies that were resolved since the last call
    pub fn close(&self) -> impl Future<Output = ()> + SendSafety + '_ {
        let mut suspended = None;
        poll_fn(move |cx| match &mut suspended {
            slot @ None => {
                let mut closing = Closing::new(self);
                let result = closing.poll(cx);
                if result.is_pending() {
                    *slot = Some(Box::new(closing));
                }
                result
            }
            Some(closing) => closing.poll(cx),
        })
    }
}

struct RunningFinalizer {
    future: BoxFuture<'static, Result<(), ()>>,
    // Keep the cloned service alive until its call completes or is cancelled.
    _service: BoxedCloneFinalizer,
    type_info: TypeInfo,
}

struct Closing<'a> {
    running: Option<RunningFinalizer>,
    remaining: Option<ResolvedSet>,
    scope: &'a ContainerInner,
    sync_scope: &'a SyncContainerInner,
    close_current_sync: bool,
}

impl<'a> Closing<'a> {
    fn new(container: &'a Container) -> Self {
        Self {
            running: None,
            remaining: container.inner.begin_close(),
            scope: container.inner.as_ref(),
            sync_scope: container.sync.inner.as_ref(),
            close_current_sync: true,
        }
    }

    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        loop {
            ready!(self.poll_finalizers(cx));
            if self.close_current_sync {
                self.sync_scope.close_with_parent_flag(false);
                self.close_current_sync = false;
            }
            if !self.scope.close_parent {
                return Poll::Ready(());
            }
            match (self.scope.parent.as_deref(), self.sync_scope.parent.as_ref()) {
                (Some(parent), Some(sync_parent)) => {
                    self.scope = parent;
                    self.sync_scope = sync_parent.inner.as_ref();
                    self.sync_scope.close_with_parent_flag(false);
                    self.remaining = parent.begin_close();
                }
                (None, None) => return Poll::Ready(()),
                _ => unreachable!(),
            }
        }
    }

    fn poll_finalizers(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        let Some(remaining) = &mut self.remaining else {
            return Poll::Ready(());
        };
        loop {
            if let Some(running) = &mut self.running {
                let _ = ready!(running.future.as_mut().poll(cx));
                let type_info = running.type_info.clone();
                self.running = None;
                debug!(%type_info, "Finalizer called");
            }

            let Some(Resolved { type_info, dependency }) = remaining.0.pop_back() else {
                self.remaining = None;
                self.scope.cache.write().map.clone_from(&self.scope.context.map);
                return Poll::Ready(());
            };
            let data = self
                .scope
                .registry
                .get_async(&type_info)
                .expect("Instantiator should be present for resolved type");
            if let Some(finalizer) = &data.finalizer {
                let mut service = finalizer.clone();
                self.running = Some(RunningFinalizer {
                    future: service.call(dependency),
                    _service: service,
                    type_info,
                });
            }
        }
    }
}

impl ContainerInner {
    fn begin_close(&self) -> Option<ResolvedSet> {
        let mut cache = self.cache.write();
        if cache.resolved.0.is_empty() {
            cache.map.clone_from(&self.context.map);
            None
        } else {
            Some(cache.take_resolved_set())
        }
    }
}
