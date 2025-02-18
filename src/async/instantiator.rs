use alloc::{boxed::Box, sync::Arc};
use core::{any::Any, future::Future};

use crate::r#async::service::{boxed_clone::BoxCloneService, fn_service::fn_service};

use super::{context::Context, dependency_resolver::DependencyResolver, registry::Registry};

#[derive(Debug)]
pub(crate) enum InstantiateErrorKind {}

pub(crate) trait Instantiator<Deps>: Clone + Send + Sync + 'static
where
    Deps: DependencyResolver<Future: Send>,
{
    type Provides: 'static;
    type Error: Into<InstantiateErrorKind>;
    type Future: Future<Output = Result<Self::Provides, Self::Error>> + Send;

    fn instantiate(&mut self, dependencies: Deps) -> Self::Future;
}

/// Config for an instantiator
/// ## Fields
/// - cache_provides:
///     If `true`, the instance provided by the instantiator will be cached and reused.
///
///     This does **not** affect the dependencies of the instance—only
///     the final result is cached if caching is applicable.
pub(crate) struct Config {
    pub(crate) cache_provides: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cache_provides: true,
        }
    }
}

pub(crate) struct Request {
    registry: Arc<Registry>,
    #[cfg(feature = "async_tokio")]
    context: Arc<tokio::sync::Mutex<Context>>,
}

impl Request {
    #[inline]
    #[must_use]
    #[cfg(feature = "async_tokio")]
    pub(crate) const fn new(
        registry: Arc<Registry>,
        context: Arc<tokio::sync::Mutex<Context>>,
    ) -> Self {
        Self { registry, context }
    }
}

#[derive(Debug)]
pub(crate) enum InstantiatorErrorKind<DepsErr, FactoryErr> {
    Deps(DepsErr),
    Factory(FactoryErr),
}

pub(crate) type BoxedCloneInstantiator<DepsErr, FactoryErr> =
    BoxCloneService<Request, Box<dyn Any>, InstantiatorErrorKind<DepsErr, FactoryErr>>;

#[must_use]
pub(crate) fn boxed_instantiator_factory<Inst, Deps>(
    instantiator: Inst,
) -> BoxedCloneInstantiator<Deps::Error, Inst::Error>
where
    Inst: Instantiator<Deps>,
    Inst::Provides: 'static,
    Deps: DependencyResolver,
{
    BoxCloneService::new(fn_service(move |Request { registry, context }| {
        let mut instantiator = instantiator.clone();

        async move {
            let dependencies = match Deps::resolve(registry, context).await {
                Ok(dependencies) => dependencies,
                Err(err) => return Err(InstantiatorErrorKind::Deps(err)),
            };
            let dependency = match instantiator.instantiate(dependencies).await {
                Ok(dependency) => dependency,
                Err(err) => return Err(InstantiatorErrorKind::Factory(err)),
            };

            Ok(Box::new(dependency) as _)
        }
    }))
}

macro_rules! impl_instantiator {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(non_snake_case)]
        impl<F, Fut, Response, Err, $($ty,)*> Instantiator<($($ty,)*)> for F
        where
            F: FnMut($($ty,)*) -> Fut + Clone + Send + Sync + 'static,
            Fut: Future<Output = Result<Response, Err>> + Send,
            Response: 'static,
            Err: Into<InstantiateErrorKind>,
            $( $ty: DependencyResolver, )*
        {
            type Provides = Response;
            type Error = Err;
            type Future = Fut;

            fn instantiate(&mut self, ($($ty,)*): ($($ty,)*)) -> Self::Future {
                self($($ty,)*)
            }
        }
    };
}

all_the_tuples!(impl_instantiator);

#[cfg(test)]
mod tests {
    extern crate std;

    use alloc::{
        format,
        string::{String, ToString as _},
    };
    use tracing::debug;
    use tracing_test::traced_test;

    use super::boxed_instantiator_factory;
    use crate::r#async::{
        context::Context, dependency_resolver::Inject, instantiator::InstantiateErrorKind,
        registry::Registry, service::base::Service as _,
    };

    #[derive(Clone, Copy)]
    struct Request(bool);
    #[derive(Clone, Copy)]
    struct Response(bool);

    #[tokio::test]
    #[traced_test]
    #[cfg(feature = "async_tokio")]
    async fn test_boxed_instantiator_factory() {
        use alloc::sync::Arc;

        let request = Request(true);

        let instantiator_request = boxed_instantiator_factory(move || async move {
            debug!("Call instantiator request");
            Ok::<_, InstantiateErrorKind>(request)
        });
        let mut instantiator_response =
            boxed_instantiator_factory(|Inject(Request(val)): Inject<_, true>| async move {
                debug!("Call instantiator response");
                Ok::<_, InstantiateErrorKind>(Response(val))
            });

        let mut registry = Registry::default();
        registry.add_instantiator::<Request>(instantiator_request);

        let response = instantiator_response
            .call(super::Request::new(
                Arc::new(registry),
                Arc::new(tokio::sync::Mutex::new(Context::default())),
            ))
            .await
            .unwrap();

        assert_eq!(request.0, response.downcast::<Response>().unwrap().0);
    }
}
