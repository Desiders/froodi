use alloc::string::ToString as _;
use core::future::Future;
use telers::{
    errors::{EventErrorKind, ExtractionError},
    event::telegram::HandlerResponse,
    middlewares::{inner::Middleware, Next},
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

macro_rules! impl_setup {
    (
        $StructName:ident,
        $ContainerType:ty
    ) => {
        #[derive(Clone)]
        pub struct $StructName<WithScope> {
            container: $ContainerType,
            scope: WithScope,
        }
    };
}

impl_setup!(ContainerMiddleware, Container);

impl<Client, WithScope> Middleware<Client> for ContainerMiddleware<WithScope>
where
    Client: Send + Sync + 'static,
    WithScope: Scope + Clone + Send + Sync + 'static,
{
    async fn call(&mut self, mut request: Request<Client>, next: Next<Client>) -> Result<HandlerResponse<Client>, EventErrorKind> {
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

        let resp = next(request).await;
        container.close();
        resp
    }
}

#[cfg(feature = "async")]
impl_setup!(AsyncContainerMiddleware, AsyncContainer);

#[cfg(feature = "async")]
impl<Client, WithScope> Middleware<Client> for AsyncContainerMiddleware<WithScope>
where
    Client: Send + Sync + 'static,
    WithScope: Scope + Clone + Send + Sync + 'static,
{
    async fn call(&mut self, mut request: Request<Client>, next: Next<Client>) -> Result<HandlerResponse<Client>, EventErrorKind> {
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

        let resp = next(request).await;
        container.close().await;
        resp
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
pub fn setup<Client, WithScope>(mut router: Router<Client>, container: Container, scope: WithScope) -> Router<Client>
where
    WithScope: Scope + Clone + Send + Sync + 'static,
    Client: Send + Sync + 'static,
{
    router.telegram_observers_mut().iter_mut().for_each(|observer| {
        observer.inner_middlewares.register(ContainerMiddleware {
            container: container.clone(),
            scope: scope.clone(),
        });
    });
    router
}

#[inline]
pub fn setup_default<Client>(router: Router<Client>, container: Container) -> Router<Client>
where
    Client: Send + Sync + 'static,
{
    setup(router, container, RequestScope)
}

#[inline]
#[cfg(feature = "async")]
pub fn setup_async<Client, WithScope>(mut router: Router<Client>, container: AsyncContainer, scope: WithScope) -> Router<Client>
where
    WithScope: Scope + Clone + Send + Sync + 'static,
    Client: Send + Sync + 'static,
{
    router.telegram_observers_mut().iter_mut().for_each(|observer| {
        observer.inner_middlewares.register(AsyncContainerMiddleware {
            container: container.clone(),
            scope: scope.clone(),
        });
    });
    router
}

#[inline]
#[cfg(feature = "async")]
pub fn setup_async_default<Client>(router: Router<Client>, container: AsyncContainer) -> Router<Client>
where
    Client: Send + Sync + 'static,
{
    setup_async(router, container, RequestScope)
}
