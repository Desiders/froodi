#[cfg(feature = "async")]
use crate::async_impl::Container as AsyncContainer;
use crate::{
    utils::future::BoxFuture,
    Container,
    DefaultScope::{Request as RequestScope, Session as SessionScope},
    Inject, InjectTransient, ResolveErrorKind, Scope,
};
use alloc::{
    boxed::Box,
    string::{String, ToString},
};
use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, HeaderMap, HeaderName, Method, Request, StatusCode, Version},
    response::{IntoResponse, Response},
    Router,
};
use core::{
    future::Future,
    str::from_utf8,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;
use tracing::error;

#[derive(Debug, thiserror::Error)]
pub enum InjectErrorKind {
    #[error("Container not found in extensions")]
    ContainerNotFound,
    #[error(transparent)]
    Resolve(ResolveErrorKind),
}

impl InjectErrorKind {
    #[inline]
    #[allow(clippy::unused_self)]
    const fn status(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }

    #[inline]
    fn body(&self) -> String {
        self.to_string()
    }
}

impl IntoResponse for InjectErrorKind {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = self.body();

        (status, body).into_response()
    }
}

macro_rules! impl_layer {
    (
        $LayerName:ident,
        $AddContainerName:ident,
        $ContainerType:ty
    ) => {
        #[allow(dead_code)]
        #[derive(Clone)]
        struct $LayerName<HScope, WSScope> {
            container: $ContainerType,
            http_scope: HScope,
            ws_scope: WSScope,
        }

        impl<S, HScope, WSScope> Layer<S> for $LayerName<HScope, WSScope>
        where
            HScope: Clone,
            WSScope: Clone,
        {
            type Service = $AddContainerName<S, HScope, WSScope>;

            fn layer(&self, service: S) -> Self::Service {
                $AddContainerName {
                    service,
                    container: self.container.clone(),
                    http_scope: self.http_scope.clone(),
                    ws_scope: self.ws_scope.clone(),
                }
            }
        }

        #[derive(Clone)]
        struct $AddContainerName<S, HScope, WSScope> {
            service: S,
            container: $ContainerType,
            http_scope: HScope,
            ws_scope: WSScope,
        }
    };
}

impl_layer!(ContainerLayer, AddContainer, Container);

impl<ResBody, S, HScope, WSScope> Service<Request<ResBody>> for AddContainer<S, HScope, WSScope>
where
    S: Service<Request<ResBody>>,
    S::Future: Send + 'static,
    HScope: Scope + Clone,
    WSScope: Scope + Clone,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, request: Request<ResBody>) -> Self::Future {
        let (parts, body) = request.into_parts();
        let is_websocket = is_websocket_request(&parts);
        let mut context = crate::Context::new();
        context.insert(parts.clone());
        let mut request = Request::from_parts(parts, body);

        let mut container_to_close = None;
        if is_websocket {
            match self
                .container
                .clone()
                .enter()
                .with_scope(self.ws_scope.clone())
                .with_context(context)
                .build()
            {
                Ok(session_container) => {
                    container_to_close = Some(session_container.clone());
                    request.extensions_mut().insert(session_container);
                }
                Err(err) => {
                    error!(%err, "Scope not found for WS request");
                }
            }
        } else {
            match self
                .container
                .clone()
                .enter()
                .with_scope(self.http_scope.clone())
                .with_context(context)
                .build()
            {
                Ok(request_container) => {
                    container_to_close = Some(request_container.clone());
                    request.extensions_mut().insert(request_container);
                }
                Err(err) => {
                    error!(%err, "Scope not found for HTTP request");
                }
            }
        }

        let future = self.service.call(request);
        Box::pin(async move {
            let response = future.await?;
            if let Some(container) = container_to_close {
                container.close();
            }
            Ok(response)
        })
    }
}

#[cfg(feature = "async")]
impl_layer!(AsyncContainerLayer, AddAsyncContainer, AsyncContainer);

#[cfg(feature = "async")]
impl<ResBody, S, HScope, WSScope> Service<Request<ResBody>> for AddAsyncContainer<S, HScope, WSScope>
where
    S: Service<Request<ResBody>>,
    S::Future: Send + 'static,
    S::Response: Send,
    HScope: Scope + Clone,
    WSScope: Scope + Clone,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, request: Request<ResBody>) -> Self::Future {
        let (parts, body) = request.into_parts();
        let is_websocket = is_websocket_request(&parts);
        let mut context = crate::Context::new();
        context.insert(parts.clone());
        let mut request = Request::from_parts(parts, body);

        let mut container_to_close = None;
        if is_websocket {
            match self
                .container
                .clone()
                .enter()
                .with_scope(self.ws_scope.clone())
                .with_context(context)
                .build()
            {
                Ok(session_container) => {
                    container_to_close = Some(session_container.clone());
                    request.extensions_mut().insert(session_container);
                }
                Err(err) => {
                    error!(%err, "Scope not found for WS request");
                }
            }
        } else {
            match self
                .container
                .clone()
                .enter()
                .with_scope(self.http_scope.clone())
                .with_context(context)
                .build()
            {
                Ok(request_container) => {
                    container_to_close = Some(request_container.clone());
                    request.extensions_mut().insert(request_container);
                }
                Err(err) => {
                    error!(%err, "Scope not found for HTTP request");
                }
            }
        }

        let future = self.service.call(request);
        Box::pin(async move {
            let response = future.await?;
            if let Some(container) = container_to_close {
                container.close().await;
            }
            Ok(response)
        })
    }
}

#[allow(clippy::manual_async_fn)]
impl<S, Dep, const PREFER_SYNC_OVER_ASYNC: bool> FromRequestParts<S> for Inject<Dep, PREFER_SYNC_OVER_ASYNC>
where
    Dep: Send + Sync + 'static,
{
    type Rejection = InjectErrorKind;

    #[cfg(not(feature = "async"))]
    fn from_request_parts(parts: &mut Parts, _state: &S) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let res = match parts.extensions.get::<Container>() {
            Some(container) => match container.get() {
                Ok(dep) => Ok(Self(dep)),
                Err(err) => Err(Self::Rejection::Resolve(err)),
            },
            None => Err(Self::Rejection::ContainerNotFound),
        };

        async move { res }
    }

    #[cfg(feature = "async")]
    fn from_request_parts(parts: &mut Parts, _state: &S) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        async move {
            if PREFER_SYNC_OVER_ASYNC {
                return match parts.extensions.get::<Container>() {
                    Some(container) => match container.get() {
                        Ok(dep) => Ok(Self(dep)),
                        Err(err) => Err(Self::Rejection::Resolve(err)),
                    },
                    None => match parts.extensions.get::<AsyncContainer>() {
                        Some(container) => match container.get().await {
                            Ok(dep) => Ok(Self(dep)),
                            Err(err) => Err(Self::Rejection::Resolve(err)),
                        },
                        None => Err(Self::Rejection::ContainerNotFound),
                    },
                };
            }

            match parts.extensions.get::<AsyncContainer>() {
                Some(container) => match container.get().await {
                    Ok(dep) => Ok(Self(dep)),
                    Err(err) => Err(Self::Rejection::Resolve(err)),
                },
                None => match parts.extensions.get::<Container>() {
                    Some(container) => match container.get() {
                        Ok(dep) => Ok(Self(dep)),
                        Err(err) => Err(Self::Rejection::Resolve(err)),
                    },
                    None => Err(Self::Rejection::ContainerNotFound),
                },
            }
        }
    }
}

#[allow(clippy::manual_async_fn)]
impl<S, Dep, const PREFER_SYNC_OVER_ASYNC: bool> FromRequestParts<S> for InjectTransient<Dep, PREFER_SYNC_OVER_ASYNC>
where
    Dep: Send + Sync + 'static,
{
    type Rejection = InjectErrorKind;

    #[cfg(not(feature = "async"))]
    fn from_request_parts(parts: &mut Parts, _state: &S) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let res = match parts.extensions.get::<Container>() {
            Some(container) => match container.get_transient() {
                Ok(dep) => Ok(Self(dep)),
                Err(err) => Err(Self::Rejection::Resolve(err)),
            },
            None => Err(Self::Rejection::ContainerNotFound),
        };

        async move { res }
    }

    #[cfg(feature = "async")]
    fn from_request_parts(parts: &mut Parts, _state: &S) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        async move {
            if PREFER_SYNC_OVER_ASYNC {
                return match parts.extensions.get::<Container>() {
                    Some(container) => match container.get_transient() {
                        Ok(dep) => Ok(Self(dep)),
                        Err(err) => Err(Self::Rejection::Resolve(err)),
                    },
                    None => match parts.extensions.get::<AsyncContainer>() {
                        Some(container) => match container.get_transient().await {
                            Ok(dep) => Ok(Self(dep)),
                            Err(err) => Err(Self::Rejection::Resolve(err)),
                        },
                        None => Err(Self::Rejection::ContainerNotFound),
                    },
                };
            }

            match parts.extensions.get::<AsyncContainer>() {
                Some(container) => match container.get_transient().await {
                    Ok(dep) => Ok(Self(dep)),
                    Err(err) => Err(Self::Rejection::Resolve(err)),
                },
                None => match parts.extensions.get::<Container>() {
                    Some(container) => match container.get_transient() {
                        Ok(dep) => Ok(Self(dep)),
                        Err(err) => Err(Self::Rejection::Resolve(err)),
                    },
                    None => Err(Self::Rejection::ContainerNotFound),
                },
            }
        }
    }
}

#[inline]
#[must_use]
fn is_websocket_request(parts: &Parts) -> bool {
    if parts.version <= Version::HTTP_11 {
        if parts.method != Method::GET {
            return false;
        }

        if !header_contains(&parts.headers, &header::CONNECTION, "upgrade") {
            return false;
        }

        if !header_eq(&parts.headers, &header::UPGRADE, "websocket") {
            return false;
        }
    } else {
        if parts.method != Method::CONNECT {
            return false;
        }

        #[cfg(feature = "http2-axum")]
        if parts
            .extensions
            .get::<h2::ext::Protocol>()
            .map_or(true, |p| p.as_str() != "websocket")
        {
            return false;
        }
    }

    true
}

#[inline]
#[must_use]
fn header_contains(headers: &HeaderMap, key: &HeaderName, value: &'static str) -> bool {
    let Some(header) = headers.get(key) else {
        return false;
    };

    if let Ok(header) = from_utf8(header.as_bytes()) {
        header.to_ascii_lowercase().contains(value)
    } else {
        false
    }
}

#[inline]
#[must_use]
fn header_eq(headers: &HeaderMap, key: &HeaderName, value: &'static str) -> bool {
    if let Some(header) = headers.get(key) {
        header.as_bytes().eq_ignore_ascii_case(value.as_bytes())
    } else {
        false
    }
}

#[inline]
pub fn setup<S, HScope, WSScope>(router: Router<S>, container: Container, http_scope: HScope, ws_scope: WSScope) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    HScope: Scope + Clone + Send + Sync + 'static,
    WSScope: Scope + Clone + Send + Sync + 'static,
{
    router.layer(ContainerLayer {
        container,
        http_scope,
        ws_scope,
    })
}

#[inline]
pub fn setup_default<S>(router: Router<S>, container: Container) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    setup(router, container, RequestScope, SessionScope)
}

#[inline]
#[cfg(feature = "async")]
pub fn setup_async<S, HScope, WSScope>(router: Router<S>, container: AsyncContainer, http_scope: HScope, ws_scope: WSScope) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    HScope: Scope + Clone + Send + Sync + 'static,
    WSScope: Scope + Clone + Send + Sync + 'static,
{
    router.layer(AsyncContainerLayer {
        container,
        http_scope,
        ws_scope,
    })
}

#[inline]
#[cfg(feature = "async")]
pub fn setup_async_default<S>(router: Router<S>, container: AsyncContainer) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    setup_async(router, container, RequestScope, SessionScope)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{setup_async_default, setup_default, AsyncContainer, Container, Inject, InjectTransient};
    use crate::{
        async_impl::RegistryBuilder as AsyncRegistriesBuilder,
        DefaultScope::{App, Request, Session},
        RegistryBuilder,
    };

    use alloc::{
        boxed::Box,
        format,
        string::{String, ToString as _},
    };
    use axum::{
        extract::ws::{Message, WebSocket, WebSocketUpgrade},
        http::request::Parts,
        response::Response,
        routing::{any, get},
        Extension, Router,
    };
    use axum_test::TestServer;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn test_container_http() {
        #[derive(Clone)]
        struct Config {
            num: i32,
        }

        #[allow(clippy::unused_async)]
        async fn handler(Extension(container): Extension<Container>, Extension(async_container): Extension<AsyncContainer>) -> Box<str> {
            (*container.get::<i32>().unwrap() + *async_container.get::<i32>().await.unwrap())
                .to_string()
                .into_boxed_str()
        }

        let container = Container::new(
            RegistryBuilder::new()
                .provide(|| Ok(Config { num: 1 }), App)
                .provide(|Inject(cfg): Inject<Config>| Ok(cfg.num + 1), Request),
        );
        let async_container = AsyncContainer::new(
            AsyncRegistriesBuilder::new()
                .provide(|| Ok(Config { num: 1 }), App)
                .provide_async(async |Inject(cfg): Inject<Config>| Ok(cfg.num + 1), Request),
        );

        let router = Router::new().route("/", get(handler));
        let router = setup_default(router, container);
        let router = setup_async_default(router, async_container);

        let server = TestServer::builder().http_transport().build(router).unwrap();

        let response = server.get("/").await;

        response.assert_status_ok();
        response.assert_text("4");
    }

    #[tokio::test]
    #[traced_test]
    async fn test_container_ws() {
        #[derive(Clone)]
        struct Config {
            num: i32,
        }

        async fn ws_upgrade(
            ws: WebSocketUpgrade,
            Extension(container): Extension<Container>,
            Extension(async_container): Extension<AsyncContainer>,
        ) -> Response {
            ws.on_upgrade(move |socket| handler(socket, container, async_container))
        }

        async fn handler(mut socket: WebSocket, container: Container, async_container: AsyncContainer) {
            while let Some(_) = socket.recv().await {
                if socket
                    .send(Message::Text(
                        (*container.get::<i32>().unwrap() + *async_container.get::<i32>().await.unwrap())
                            .to_string()
                            .into(),
                    ))
                    .await
                    .is_err()
                {
                    return;
                }
            }
        }

        let container = Container::new(
            RegistryBuilder::new()
                .provide(|| Ok(Config { num: 1 }), App)
                .provide(|Inject(cfg): Inject<Config>| Ok(cfg.num + 1), Session),
        );
        let async_container = AsyncContainer::new(
            AsyncRegistriesBuilder::new()
                .provide(|| Ok(Config { num: 1 }), App)
                .provide_async(async |Inject(cfg): Inject<Config>| Ok(cfg.num + 1), Session),
        );

        let router = Router::new().route("/", any(ws_upgrade));
        let router = setup_default(router, container);
        let router = setup_async_default(router, async_container);

        let server = TestServer::builder().http_transport().build(router).unwrap();

        let mut ws = server.get_websocket("/").await.into_websocket().await;

        ws.send_text("").await;
        ws.assert_receive_text("4").await;
    }

    #[tokio::test]
    #[traced_test]
    async fn test_dep_inject() {
        #[derive(Clone)]
        struct Config {
            num: i32,
        }

        #[allow(clippy::unused_async)]
        async fn handler(Inject(_config): Inject<Config>, InjectTransient(num): InjectTransient<i32>) -> Box<str> {
            num.to_string().into_boxed_str()
        }

        let container = Container::new(RegistryBuilder::new().provide(|| Ok(Config { num: 1 }), App).provide(
            |Inject(cfg): Inject<Config>, Inject(_parts): Inject<Parts>| Ok(cfg.num + 1),
            Request,
        ));

        let router = setup_default(Router::new().route("/", get(handler)), container);

        let server = TestServer::builder().http_transport().build(router).unwrap();

        let response = server.get("/").await;

        response.assert_status_ok();
        response.assert_text("2");
    }
}
