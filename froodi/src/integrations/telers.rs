use alloc::string::ToString as _;
use core::future::Future;
use telers::{
    errors::ExtractionError,
    middlewares::{OuterMiddleware, OuterMiddlewareResult, OuterNext},
    Extractor, Request, Router,
};

#[cfg(feature = "async")]
use crate::async_impl::Container as AsyncContainer;
use crate::{Container, Context, DefaultScope::Request as RequestScope, Inject, InjectTransient, ResolveErrorKind, Scope};

#[derive(Debug, thiserror::Error)]
pub enum InjectErrorKind {
    #[error("Container not found in extensions")]
    ContainerNotFound,
    #[error(transparent)]
    Resolve(ResolveErrorKind),
}

impl From<InjectErrorKind> for ExtractionError {
    fn from(value: InjectErrorKind) -> Self {
        Self::new(value.to_string())
    }
}

#[derive(Clone)]
pub struct ContainerOuterMiddleware<WithScope> {
    container: Container,
    scope: WithScope,
}

impl<Client, WithScope> OuterMiddleware<Client> for ContainerOuterMiddleware<WithScope>
where
    Client: Send + Sync + 'static,
    WithScope: Scope + Clone + Send + Sync + 'static,
{
    fn call(
        &mut self,
        mut request: Request<Client>,
        next: OuterNext<Client>,
    ) -> impl Future<Output = OuterMiddlewareResult<Client>> + Send {
        let mut context = Context::new();
        context.insert(request.update.clone());

        let container = self
            .container
            .clone()
            .enter()
            .with_scope(self.scope.clone())
            .with_context(context)
            .build()
            .unwrap();
        request.extensions.insert(container.clone());

        async move {
            let result = next(request).await;
            container.close();
            result
        }
    }
}

#[cfg(feature = "async")]
#[derive(Clone)]
pub struct AsyncContainerOuterMiddleware<WithScope> {
    container: AsyncContainer,
    scope: WithScope,
}

#[cfg(feature = "async")]
impl<Client, WithScope> OuterMiddleware<Client> for AsyncContainerOuterMiddleware<WithScope>
where
    Client: Send + Sync + 'static,
    WithScope: Scope + Clone + Send + Sync + 'static,
{
    fn call(
        &mut self,
        mut request: Request<Client>,
        next: OuterNext<Client>,
    ) -> impl Future<Output = OuterMiddlewareResult<Client>> + Send {
        let mut context = Context::new();
        context.insert(request.update.clone());

        let container = self
            .container
            .clone()
            .enter()
            .with_scope(self.scope.clone())
            .with_context(context)
            .build()
            .unwrap();
        request.extensions.insert(container.clone());

        async move {
            let result = next(request).await;
            container.close().await;
            result
        }
    }
}

impl<Client, Dep, const PREFER_SYNC_OVER_ASYNC: bool> Extractor<Client> for Inject<Dep, PREFER_SYNC_OVER_ASYNC>
where
    Dep: Send + Sync + 'static,
{
    type Error = InjectErrorKind;

    #[cfg(not(feature = "async"))]
    fn extract(request: &Request<Client>) -> impl Future<Output = Result<Self, Self::Error>> + Send {
        let res = match request.extensions.get::<Container>() {
            Some(container) => match container.get() {
                Ok(dep) => Ok(Self(dep)),
                Err(err) => Err(Self::Error::Resolve(err)),
            },
            None => Err(Self::Error::ContainerNotFound),
        };
        async move { res }
    }

    #[cfg(feature = "async")]
    fn extract(request: &Request<Client>) -> impl Future<Output = Result<Self, Self::Error>> + Send {
        let sync_container = request.extensions.get::<Container>();
        let async_container = request.extensions.get::<AsyncContainer>();
        async move {
            if PREFER_SYNC_OVER_ASYNC {
                return match sync_container {
                    Some(container) => match container.get() {
                        Ok(dep) => Ok(Self(dep)),
                        Err(err) => Err(Self::Error::Resolve(err)),
                    },
                    None => match async_container {
                        Some(container) => match container.get().await {
                            Ok(dep) => Ok(Self(dep)),
                            Err(err) => Err(Self::Error::Resolve(err)),
                        },
                        None => Err(Self::Error::ContainerNotFound),
                    },
                };
            }

            match async_container {
                Some(container) => match container.get().await {
                    Ok(dep) => Ok(Self(dep)),
                    Err(err) => Err(Self::Error::Resolve(err)),
                },
                None => match sync_container {
                    Some(container) => match container.get() {
                        Ok(dep) => Ok(Self(dep)),
                        Err(err) => Err(Self::Error::Resolve(err)),
                    },
                    None => Err(Self::Error::ContainerNotFound),
                },
            }
        }
    }
}

impl<Client, Dep, const PREFER_SYNC_OVER_ASYNC: bool> Extractor<Client> for InjectTransient<Dep, PREFER_SYNC_OVER_ASYNC>
where
    Dep: Send + Sync + 'static,
{
    type Error = InjectErrorKind;

    #[cfg(not(feature = "async"))]
    fn extract(request: &Request<Client>) -> impl Future<Output = Result<Self, Self::Error>> + Send {
        let res = match request.extensions.get::<Container>() {
            Some(container) => match container.get_transient() {
                Ok(dep) => Ok(Self(dep)),
                Err(err) => Err(Self::Error::Resolve(err)),
            },
            None => Err(Self::Error::ContainerNotFound),
        };
        async move { res }
    }

    #[cfg(feature = "async")]
    fn extract(request: &Request<Client>) -> impl Future<Output = Result<Self, Self::Error>> + Send {
        let sync_container = request.extensions.get::<Container>();
        let async_container = request.extensions.get::<AsyncContainer>();
        async move {
            if PREFER_SYNC_OVER_ASYNC {
                return match sync_container {
                    Some(container) => match container.get_transient() {
                        Ok(dep) => Ok(Self(dep)),
                        Err(err) => Err(Self::Error::Resolve(err)),
                    },
                    None => match async_container {
                        Some(container) => match container.get_transient().await {
                            Ok(dep) => Ok(Self(dep)),
                            Err(err) => Err(Self::Error::Resolve(err)),
                        },
                        None => Err(Self::Error::ContainerNotFound),
                    },
                };
            }

            match async_container {
                Some(container) => match container.get_transient().await {
                    Ok(dep) => Ok(Self(dep)),
                    Err(err) => Err(Self::Error::Resolve(err)),
                },
                None => match sync_container {
                    Some(container) => match container.get_transient() {
                        Ok(dep) => Ok(Self(dep)),
                        Err(err) => Err(Self::Error::Resolve(err)),
                    },
                    None => Err(Self::Error::ContainerNotFound),
                },
            }
        }
    }
}

#[inline]
#[must_use]
pub fn setup<Client, WithScope>(router: Router<Client>, container: Container, scope: WithScope) -> Router<Client>
where
    WithScope: Scope + Clone + Send + Sync + 'static,
    Client: Send + Sync + 'static,
{
    router.on_update_middleware(ContainerOuterMiddleware { container, scope })
}

#[inline]
#[must_use]
pub fn setup_default<Client>(router: Router<Client>, container: Container) -> Router<Client>
where
    Client: Send + Sync + 'static,
{
    setup(router, container, RequestScope)
}

#[inline]
#[must_use]
#[cfg(feature = "async")]
pub fn setup_async<Client, WithScope>(router: Router<Client>, container: AsyncContainer, scope: WithScope) -> Router<Client>
where
    WithScope: Scope + Clone + Send + Sync + 'static,
    Client: Send + Sync + 'static,
{
    router.on_update_middleware(AsyncContainerOuterMiddleware { container, scope })
}

#[inline]
#[must_use]
#[cfg(feature = "async")]
pub fn setup_async_default<Client>(router: Router<Client>, container: AsyncContainer) -> Router<Client>
where
    Client: Send + Sync + 'static,
{
    setup_async(router, container, RequestScope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{registry, InstantiateErrorKind};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use telers::{
        client::Reqwest,
        errors::EventErrorKind,
        event::bases::PropagateEventResult,
        router::Response,
        types::{ChatPrivate, MessageText, Update, UpdateMessage},
        Bot, Extensions,
    };

    fn request() -> Request<Reqwest> {
        Request {
            update: Arc::new(Update::Message(UpdateMessage::new(
                0,
                MessageText::new(0, 0, ChatPrivate::new(0), ""),
            ))),
            bot: Bot::default(),
            context: telers::Context::default(),
            extensions: Extensions::default(),
        }
    }

    #[tokio::test]
    async fn closes_request_scope_after_unhandled_and_failed_routing() {
        let finalized = Arc::new(AtomicUsize::new(0));
        let finalizer_count = finalized.clone();
        let container = Container::new(registry! {
            provide(RequestScope, || Ok::<_, InstantiateErrorKind>(42u32), finalizer = move |_: Arc<u32>| {
                finalizer_count.fetch_add(1, Ordering::SeqCst);
            }),
        });
        let mut middleware = ContainerOuterMiddleware {
            container,
            scope: RequestScope,
        };

        let unhandled: OuterNext<Reqwest> = Box::new(|request| {
            Box::pin(async move {
                let request_container = request.extensions.get::<Container>().unwrap();
                assert_eq!(*request_container.get::<u32>().unwrap(), 42);
                Ok(Response::new(request, PropagateEventResult::Unhandled))
            })
        });
        assert!(middleware.call(request(), unhandled).await.is_ok());
        assert_eq!(finalized.load(Ordering::SeqCst), 1);

        let failed: OuterNext<Reqwest> = Box::new(|request| {
            Box::pin(async move {
                let request_container = request.extensions.get::<Container>().unwrap();
                assert_eq!(*request_container.get::<u32>().unwrap(), 42);
                Err((EventErrorKind::Extraction(ExtractionError::new("test")), request))
            })
        });
        assert!(middleware.call(request(), failed).await.is_err());
        assert_eq!(finalized.load(Ordering::SeqCst), 2);
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn closes_async_request_scope_after_unhandled_routing() {
        let finalized = Arc::new(AtomicUsize::new(0));
        let finalizer_count = finalized.clone();
        let container = AsyncContainer::new(registry! {
            provide(RequestScope, || async { Ok::<_, InstantiateErrorKind>(42u32) }, finalizer = move |_: Arc<u32>| {
                let finalizer_count = finalizer_count.clone();
                async move {
                    finalizer_count.fetch_add(1, Ordering::SeqCst);
                }
            }),
        });
        let mut middleware = AsyncContainerOuterMiddleware {
            container,
            scope: RequestScope,
        };

        let unhandled: OuterNext<Reqwest> = Box::new(|request| {
            Box::pin(async move {
                let request_container = request.extensions.get::<AsyncContainer>().unwrap();
                assert_eq!(*request_container.get::<u32>().await.unwrap(), 42);
                Ok(Response::new(request, PropagateEventResult::Unhandled))
            })
        });
        assert!(middleware.call(request(), unhandled).await.is_ok());
        assert_eq!(finalized.load(Ordering::SeqCst), 1);
    }
}
