use alloc::boxed::Box;
use async_trait::async_trait;
use telers::{
    errors::{EventErrorKind, ExtractionError},
    event::telegram::HandlerResponse,
    middlewares::{inner::Middleware, Next},
    Extractor, Request, Router,
};

use crate::{Container, Context, DefaultScope::Request as RequestScope, Inject, Scope};

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

#[async_trait]
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

impl<Client, Dep, const PREFER_SYNC_OVER_ASYNC: bool> Extractor<Client> for Inject<Dep, PREFER_SYNC_OVER_ASYNC>
where
    Dep: Send + Sync + 'static,
{
    type Error = ExtractionError;

    fn extract(request: &Request<Client>) -> Result<Self, Self::Error> {
        use alloc::string::ToString as _;
        match request.extensions.get::<Container>() {
            Some(container) => match container.get() {
                Ok(dep) => Ok(Self(dep)),
                Err(err) => Err(Self::Error::new(err.to_string())),
            },
            None => Err(Self::Error::new("Container not found in extensions")),
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
