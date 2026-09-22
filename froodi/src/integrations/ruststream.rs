//! `ruststream` integration: a delivery runs inside a froodi scope, and a handler takes its
//! dependencies as [`Inject`] / [`InjectTransient`] parameters.
//!
//! [`ContainerLayer`] holds a sync [`Container`], [`AsyncContainerLayer`] (feature `async`) an async one.
//! Both ride `RustStream::layer(..)`: for every delivery the layer enters a child scope of its container
//! (`Request` unless [`with_scope`](ContainerLayer::with_scope) names another), runs the handler inside it and
//! closes the child once the handler has answered, so finalizers run before the message is settled.
//!
//! ```
//! use froodi::{ruststream::ContainerLayer, Container, DefaultScope::Request, Inject, registry};
//! use ruststream::memory::prelude::*;
//!
//! struct Greeter;
//!
//! #[subscriber("greetings")]
//! async fn greet(name: &String, Inject(greeter): Inject<Greeter>) -> HandlerOutcome {
//!     HandlerOutcome::ack()
//! }
//!
//! let container = Container::new(registry! {
//!     scope(Request) [
//!         provide(|| Ok(Greeter)),
//!     ],
//! });
//! let app = RustStream::new(AppInfo::new("greetings", "0.1.0"))
//!     .layer(ContainerLayer::new(container))
//!     .with_broker(MemoryBroker::new(), |b| {
//!         b.include(greet);
//!     });
//! ```
//!
//! The child is closed by the layer's own future: a delivery cancelled mid-flight (the shutdown timeout) or a
//! handler panic caught by the runtime skips the finalizers. A handler taking [`Inject`] in a service that mounts
//! no layer drops every delivery and logs an error for each: the extractor sees only the delivery context, so a
//! missing layer cannot be rejected at compile time or at startup.

// `tokio::task_local!` expands to `std::thread_local!`, which the `no_std` crate root does not link.
extern crate std;

use core::any::type_name;
#[cfg(feature = "async")]
use core::future::Future;

use ruststream::runtime::{BlanketLayer, Context, FromContext, Handler, HandlerOutcome, Layer};
use tracing::error;

#[cfg(feature = "async")]
use crate::async_impl::Container as AsyncContainer;
use crate::{
    Container, DefaultScope, DefaultScope::Request as RequestScope, Inject, InjectTransient, ResolveErrorKind, Scope, ScopeWithErrorKind,
};

// The task is the only per-delivery slot both the layer and the extractor can reach: the runtime's
// `Context` carries no type-erased extensions, and its typed scratch is keyed by the broker's own
// context type, which knows nothing of froodi.
tokio::task_local! {
    /// The child container [`ContainerLayer`] entered for the in-flight delivery.
    static CONTAINER: Container;

    /// The child container [`AsyncContainerLayer`] entered for the in-flight delivery.
    #[cfg(feature = "async")]
    static ASYNC_CONTAINER: AsyncContainer;
}

#[derive(Debug, thiserror::Error)]
pub enum InjectErrorKind {
    #[error("Container not found in the delivery's task: the service mounts no `ContainerLayer` or `AsyncContainerLayer`")]
    ContainerNotFound,
    #[error(transparent)]
    Resolve(ResolveErrorKind),
}

/// A failed injection drops the delivery (a nack without requeue); the extractor has logged the cause.
impl From<InjectErrorKind> for HandlerOutcome {
    fn from(_: InjectErrorKind) -> Self {
        HandlerOutcome::drop()
    }
}

macro_rules! impl_layer {
    (
        $(#[$layer_doc:meta])*
        $LayerName:ident,
        $(#[$handler_doc:meta])*
        $HandlerName:ident,
        $ContainerType:ty
    ) => {
        $(#[$layer_doc])*
        #[derive(Clone)]
        pub struct $LayerName<WithScope = DefaultScope> {
            container: $ContainerType,
            scope: WithScope,
        }

        impl $LayerName {
            /// Enters the `Request` scope of `container` for every delivery.
            #[inline]
            #[must_use]
            pub fn new(container: $ContainerType) -> Self {
                Self::with_scope(container, RequestScope)
            }
        }

        impl<WithScope> $LayerName<WithScope> {
            /// Enters `scope` of `container` for every delivery.
            #[inline]
            #[must_use]
            pub fn with_scope(container: $ContainerType, scope: WithScope) -> Self {
                Self { container, scope }
            }
        }

        impl<H, WithScope: Clone> Layer<H> for $LayerName<WithScope> {
            type Handler = $HandlerName<H, WithScope>;

            fn layer(&self, inner: H) -> Self::Handler {
                $HandlerName {
                    inner,
                    container: self.container.clone(),
                    scope: self.scope.clone(),
                }
            }
        }

        impl<WithScope> BlanketLayer for $LayerName<WithScope>
        where
            WithScope: Scope + Clone + Send + Sync + 'static,
        {
            fn apply<M, C, S, H>(&self, handler: H) -> impl Handler<M, C, S> + 'static
            where
                M: Send + Sync + 'static,
                C: Send + 'static,
                S: Send + Sync + 'static,
                H: Handler<M, C, S> + 'static,
            {
                self.layer(handler)
            }
        }

        $(#[$handler_doc])*
        #[derive(Clone)]
        pub struct $HandlerName<H, WithScope> {
            inner: H,
            container: $ContainerType,
            scope: WithScope,
        }
    };
}

impl_layer!(
    /// Middleware that runs every handler inside a child scope of a sync [`Container`].
    ///
    /// Mount it with `RustStream::layer(..)` (or `Router::layer(..)`); it wraps handlers registered directly and
    /// through a router alike. The child is entered before the handler and closed after it has answered.
    ContainerLayer,
    /// A handler wrapped by [`ContainerLayer`].
    WithContainer,
    Container
);

#[cfg(feature = "async")]
impl_layer!(
    /// Middleware that runs every handler inside a child scope of an async [`AsyncContainer`].
    ///
    /// Mount it with `RustStream::layer(..)` (or `Router::layer(..)`); it wraps handlers registered directly and
    /// through a router alike. The child is entered before the handler and closed after it has answered, so async
    /// finalizers complete before the message is settled.
    AsyncContainerLayer,
    /// A handler wrapped by [`AsyncContainerLayer`].
    WithAsyncContainer,
    AsyncContainer
);

impl<M, C, S, H, WithScope> Handler<M, C, S> for WithContainer<H, WithScope>
where
    M: ?Sized + Send + Sync,
    C: Send,
    S: Send + Sync,
    H: Handler<M, C, S>,
    WithScope: Scope + Clone + Send + Sync,
{
    async fn handle(&self, msg: &M, ctx: &mut Context<'_, C, S>) -> HandlerOutcome {
        let child = match self.container.clone().enter().with_scope(self.scope.clone()).build() {
            Ok(child) => child,
            Err(err) => return scope_not_found(ctx.name(), &err),
        };
        let outcome = CONTAINER.scope(child.clone(), self.inner.handle(msg, ctx)).await;
        child.close();
        outcome
    }
}

#[cfg(feature = "async")]
impl<M, C, S, H, WithScope> Handler<M, C, S> for WithAsyncContainer<H, WithScope>
where
    M: ?Sized + Send + Sync,
    C: Send,
    S: Send + Sync,
    H: Handler<M, C, S>,
    WithScope: Scope + Clone + Send + Sync,
{
    async fn handle(&self, msg: &M, ctx: &mut Context<'_, C, S>) -> HandlerOutcome {
        let child = match self.container.clone().enter().with_scope(self.scope.clone()).build() {
            Ok(child) => child,
            Err(err) => return scope_not_found(ctx.name(), &err),
        };
        let outcome = ASYNC_CONTAINER.scope(child.clone(), self.inner.handle(msg, ctx)).await;
        child.close().await;
        outcome
    }
}

impl<C, S, Dep, const PREFER_SYNC_OVER_ASYNC: bool> FromContext<C, S> for Inject<Dep, PREFER_SYNC_OVER_ASYNC>
where
    C: Send,
    S: Sync,
    Dep: Send + Sync + 'static,
{
    type Rejection = InjectErrorKind;

    async fn from_context(ctx: &mut Context<'_, C, S>) -> Result<Self, InjectErrorKind> {
        #[cfg(feature = "async")]
        let resolved = resolve(PREFER_SYNC_OVER_ASYNC, Container::get::<Dep>, |container| async move {
            container.get::<Dep>().await
        })
        .await;
        #[cfg(not(feature = "async"))]
        let resolved = resolve(Container::get::<Dep>);
        resolved.map(Self).map_err(|err| rejected::<Dep>(ctx.name(), err))
    }
}

impl<C, S, Dep, const PREFER_SYNC_OVER_ASYNC: bool> FromContext<C, S> for InjectTransient<Dep, PREFER_SYNC_OVER_ASYNC>
where
    C: Send,
    S: Sync,
    Dep: Send + Sync + 'static,
{
    type Rejection = InjectErrorKind;

    async fn from_context(ctx: &mut Context<'_, C, S>) -> Result<Self, InjectErrorKind> {
        #[cfg(feature = "async")]
        let resolved = resolve(PREFER_SYNC_OVER_ASYNC, Container::get_transient::<Dep>, |container| async move {
            container.get_transient::<Dep>().await
        })
        .await;
        #[cfg(not(feature = "async"))]
        let resolved = resolve(Container::get_transient::<Dep>);
        resolved.map(Self).map_err(|err| rejected::<Dep>(ctx.name(), err))
    }
}

/// Resolves through the containers the task carries: `sync_get` against the sync one, `async_get` against the
/// async one, in the order `prefer_sync` names. A container no layer entered is skipped; both missing is
/// [`InjectErrorKind::ContainerNotFound`].
#[cfg(feature = "async")]
async fn resolve<Dep, SyncGet, AsyncGet, AsyncFut>(
    prefer_sync: bool,
    sync_get: SyncGet,
    async_get: AsyncGet,
) -> Result<Dep, InjectErrorKind>
where
    SyncGet: FnOnce(&Container) -> Result<Dep, ResolveErrorKind>,
    AsyncGet: FnOnce(AsyncContainer) -> AsyncFut,
    AsyncFut: Future<Output = Result<Dep, ResolveErrorKind>>,
{
    let async_container = ASYNC_CONTAINER.try_with(Clone::clone).ok();
    if prefer_sync {
        if let Ok(resolved) = CONTAINER.try_with(sync_get) {
            return resolved.map_err(InjectErrorKind::Resolve);
        }
        return match async_container {
            Some(container) => async_get(container).await.map_err(InjectErrorKind::Resolve),
            None => Err(InjectErrorKind::ContainerNotFound),
        };
    }
    if let Some(container) = async_container {
        return async_get(container).await.map_err(InjectErrorKind::Resolve);
    }
    CONTAINER
        .try_with(sync_get)
        .map_err(|_| InjectErrorKind::ContainerNotFound)?
        .map_err(InjectErrorKind::Resolve)
}

/// Resolves through the sync container the task carries; none is [`InjectErrorKind::ContainerNotFound`].
#[cfg(not(feature = "async"))]
fn resolve<Dep, SyncGet>(sync_get: SyncGet) -> Result<Dep, InjectErrorKind>
where
    SyncGet: FnOnce(&Container) -> Result<Dep, ResolveErrorKind>,
{
    CONTAINER
        .try_with(sync_get)
        .map_err(|_| InjectErrorKind::ContainerNotFound)?
        .map_err(InjectErrorKind::Resolve)
}

/// Logs a failed injection with the channel and the dependency it was for and hands the error back as the
/// rejection; the runtime settles a rejected delivery without logging it.
fn rejected<Dep>(channel: &str, err: InjectErrorKind) -> InjectErrorKind {
    error!(%err, channel, dependency = type_name::<Dep>(), "Dependency injection failed, the delivery is dropped");
    err
}

fn scope_not_found(channel: &str, err: &ScopeWithErrorKind) -> HandlerOutcome {
    error!(%err, channel, "Scope not found for the delivery, the delivery is dropped");
    HandlerOutcome::drop()
}

#[cfg(test)]
mod tests {
    extern crate std;

    use alloc::string::String;
    use core::sync::atomic::{AtomicUsize, Ordering};

    use ruststream::memory::prelude::*;
    use ruststream::testing::TestApp;

    #[cfg(feature = "async")]
    use super::{AsyncContainer, AsyncContainerLayer};
    use super::{Container, ContainerLayer, Inject, InjectTransient};
    #[cfg(feature = "async")]
    use crate::async_registry;
    use crate::{
        instance, registry,
        utils::thread_safety::RcThreadSafety,
        DefaultScope::{App, Request, Session},
    };

    /// What the handlers observed. Lives in the App scope, so a handler reaches it through injection.
    #[derive(Clone, Default)]
    struct Counters {
        built: RcThreadSafety<AtomicUsize>,
        finalized: RcThreadSafety<AtomicUsize>,
        hits: RcThreadSafety<AtomicUsize>,
        shared: RcThreadSafety<AtomicUsize>,
    }

    impl Counters {
        fn built(&self) -> usize {
            self.built.load(Ordering::SeqCst)
        }

        fn finalized(&self) -> usize {
            self.finalized.load(Ordering::SeqCst)
        }

        fn hits(&self) -> usize {
            self.hits.load(Ordering::SeqCst)
        }

        fn shared(&self) -> usize {
            self.shared.load(Ordering::SeqCst)
        }
    }

    struct Greeter {
        counters: Counters,
    }

    struct SessionGreeter;

    struct Unregistered;

    fn build_container(counters: &Counters) -> Container {
        Container::new(registry! {
            provide(App, instance(counters.clone())),
            scope(Session) [
                provide(|| Ok(SessionGreeter)),
            ],
            scope(Request) [
                provide(
                    |Inject(counters): Inject<Counters>| {
                        counters.built.fetch_add(1, Ordering::SeqCst);
                        Ok(Greeter { counters: (*counters).clone() })
                    },
                    finalizer = |greeter: RcThreadSafety<Greeter>| {
                        greeter.counters.finalized.fetch_add(1, Ordering::SeqCst);
                    }
                ),
            ],
        })
    }

    #[cfg(feature = "async")]
    fn build_async_container(counters: &Counters) -> AsyncContainer {
        AsyncContainer::new(async_registry! {
            scope(Request) [
                provide(
                    |Inject(counters): Inject<Counters>| async move {
                        counters.built.fetch_add(1, Ordering::SeqCst);
                        Ok(Greeter { counters: (*counters).clone() })
                    },
                    finalizer = |greeter: RcThreadSafety<Greeter>| async move {
                        greeter.counters.finalized.fetch_add(1, Ordering::SeqCst);
                    }
                ),
            ],
            extend(registry! {
                provide(App, instance(counters.clone())),
            }),
        })
    }

    #[subscriber("greetings")]
    async fn greet(_name: &String, Inject(greeter): Inject<Greeter>) -> HandlerOutcome {
        greeter.counters.hits.fetch_add(1, Ordering::SeqCst);
        HandlerOutcome::ack()
    }

    #[subscriber("pairs")]
    async fn pair(_name: &String, Inject(first): Inject<Greeter>, Inject(second): Inject<Greeter>) -> HandlerOutcome {
        if RcThreadSafety::ptr_eq(&first, &second) {
            first.counters.shared.fetch_add(1, Ordering::SeqCst);
        }
        HandlerOutcome::ack()
    }

    #[subscriber("transients")]
    async fn transients(
        _name: &String,
        InjectTransient(first): InjectTransient<Greeter>,
        InjectTransient(_second): InjectTransient<Greeter>,
    ) -> HandlerOutcome {
        first.counters.hits.fetch_add(1, Ordering::SeqCst);
        HandlerOutcome::ack()
    }

    #[subscriber("sessions")]
    async fn session(_name: &String, Inject(_greeter): Inject<SessionGreeter>, Inject(counters): Inject<Counters>) -> HandlerOutcome {
        counters.hits.fetch_add(1, Ordering::SeqCst);
        HandlerOutcome::ack()
    }

    #[subscriber("orphans")]
    async fn orphan(_name: &String, Inject(_missing): Inject<Unregistered>) -> HandlerOutcome {
        HandlerOutcome::ack()
    }

    async fn publish(tb: &TestApp<()>, channel: &str) {
        tb.broker::<MemoryBroker>()
            .message(&String::from("froodi"))
            .to(channel)
            .publish()
            .await
            .expect("publish");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn each_delivery_gets_its_own_scope_and_finalizers_run() {
        let counters = Counters::default();
        let app = RustStream::new(AppInfo::new("svc", "0.1.0"))
            .layer(ContainerLayer::new(build_container(&counters)))
            .with_broker(MemoryBroker::new(), |b| {
                b.include(greet);
            });
        let tb = TestApp::start(app).await.expect("start");

        publish(&tb, "greetings").await;
        publish(&tb, "greetings").await;

        tb.broker::<MemoryBroker>()
            .subscriber("greetings")
            .assert_called(2)
            .settled(HandlerOutcome::ack());
        assert_eq!(counters.hits(), 2);
        assert_eq!(counters.built(), 2);
        assert_eq!(counters.finalized(), 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn injections_of_one_delivery_share_the_scope() {
        let counters = Counters::default();
        let app = RustStream::new(AppInfo::new("svc", "0.1.0"))
            .layer(ContainerLayer::new(build_container(&counters)))
            .with_broker(MemoryBroker::new(), |b| {
                b.include(pair);
            });
        let tb = TestApp::start(app).await.expect("start");

        publish(&tb, "pairs").await;

        tb.broker::<MemoryBroker>()
            .subscriber("pairs")
            .assert_called_once()
            .settled(HandlerOutcome::ack());
        assert_eq!(counters.built(), 1);
        assert_eq!(counters.shared(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn transient_injection_builds_a_fresh_instance() {
        let counters = Counters::default();
        let app = RustStream::new(AppInfo::new("svc", "0.1.0"))
            .layer(ContainerLayer::new(build_container(&counters)))
            .with_broker(MemoryBroker::new(), |b| {
                b.include(transients);
            });
        let tb = TestApp::start(app).await.expect("start");

        publish(&tb, "transients").await;

        tb.broker::<MemoryBroker>()
            .subscriber("transients")
            .assert_called_once()
            .settled(HandlerOutcome::ack());
        assert_eq!(counters.hits(), 1);
        assert_eq!(counters.built(), 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn layer_enters_the_named_scope() {
        let counters = Counters::default();
        let app = RustStream::new(AppInfo::new("svc", "0.1.0"))
            .layer(ContainerLayer::with_scope(build_container(&counters), Session))
            .with_broker(MemoryBroker::new(), |b| {
                b.include(session);
            });
        let tb = TestApp::start(app).await.expect("start");

        publish(&tb, "sessions").await;

        tb.broker::<MemoryBroker>()
            .subscriber("sessions")
            .assert_called_once()
            .settled(HandlerOutcome::ack());
        assert_eq!(counters.hits(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn router_mounted_handler_runs_in_the_scope() {
        let counters = Counters::default();
        let app = RustStream::new(AppInfo::new("svc", "0.1.0"))
            .layer(ContainerLayer::new(build_container(&counters)))
            .with_broker(MemoryBroker::new(), |b| {
                b.include_router(Router::new().include(greet));
            });
        let tb = TestApp::start(app).await.expect("start");

        publish(&tb, "greetings").await;

        tb.broker::<MemoryBroker>()
            .subscriber("greetings")
            .assert_called_once()
            .settled(HandlerOutcome::ack());
        assert_eq!(counters.hits(), 1);
        assert_eq!(counters.built(), 1);
        assert_eq!(counters.finalized(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn missing_layer_drops_the_delivery() {
        let counters = Counters::default();
        let _container = build_container(&counters);
        let app = RustStream::new(AppInfo::new("svc", "0.1.0")).with_broker(MemoryBroker::new(), |b| {
            b.include(greet);
        });
        let tb = TestApp::start(app).await.expect("start");

        publish(&tb, "greetings").await;

        tb.broker::<MemoryBroker>()
            .subscriber("greetings")
            .assert_called_once()
            .settled(HandlerOutcome::drop());
        assert_eq!(counters.hits(), 0);
        assert_eq!(counters.built(), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unregistered_dependency_drops_the_delivery() {
        let counters = Counters::default();
        let app = RustStream::new(AppInfo::new("svc", "0.1.0"))
            .layer(ContainerLayer::new(build_container(&counters)))
            .with_broker(MemoryBroker::new(), |b| {
                b.include(orphan);
            });
        let tb = TestApp::start(app).await.expect("start");

        publish(&tb, "orphans").await;

        tb.broker::<MemoryBroker>()
            .subscriber("orphans")
            .assert_called_once()
            .settled(HandlerOutcome::drop());
    }

    #[cfg(feature = "async")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn async_container_scopes_each_delivery_and_runs_async_finalizers() {
        let counters = Counters::default();
        let app = RustStream::new(AppInfo::new("svc", "0.1.0"))
            .layer(AsyncContainerLayer::new(build_async_container(&counters)))
            .with_broker(MemoryBroker::new(), |b| {
                b.include(greet);
                b.include(pair);
            });
        let tb = TestApp::start(app).await.expect("start");

        publish(&tb, "greetings").await;
        publish(&tb, "greetings").await;
        publish(&tb, "pairs").await;

        tb.broker::<MemoryBroker>()
            .subscriber("greetings")
            .assert_called(2)
            .settled(HandlerOutcome::ack());
        tb.broker::<MemoryBroker>()
            .subscriber("pairs")
            .assert_called_once()
            .settled(HandlerOutcome::ack());
        assert_eq!(counters.hits(), 2);
        assert_eq!(counters.built(), 3);
        assert_eq!(counters.finalized(), 3);
        assert_eq!(counters.shared(), 1);
    }

    /// `Inject<Dep>` prefers the sync container, and with no sync layer mounted it falls back to the async one.
    #[cfg(feature = "async")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sync_preference_falls_back_to_the_async_container() {
        let counters = Counters::default();
        let app = RustStream::new(AppInfo::new("svc", "0.1.0"))
            .layer(AsyncContainerLayer::new(build_async_container(&counters)))
            .with_broker(MemoryBroker::new(), |b| {
                b.include(transients);
            });
        let tb = TestApp::start(app).await.expect("start");

        publish(&tb, "transients").await;

        tb.broker::<MemoryBroker>()
            .subscriber("transients")
            .assert_called_once()
            .settled(HandlerOutcome::ack());
        assert_eq!(counters.built(), 2);
    }
}
