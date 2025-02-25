use alloc::{boxed::Box, sync::Arc};
use core::{
    any::{type_name, Any},
    future::Future,
};
use tracing::{debug, debug_span, Instrument as _};

use super::{context::Context, dependency_resolver::DependencyResolver, registry::Registry};
use crate::{
    errors::{InstantiateErrorKind, InstantiatorErrorKind},
    r#async::service::{service_fn, BoxCloneService},
};

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
/// - `cache_provides`:
///     If `true`, the instance provided by the instantiator will be cached and reused.
///
///     This does **not** affect the dependencies of the instance.
///     Only the final result is cached if caching is applicable.
#[derive(Clone, Copy)]
pub(crate) struct Config {
    pub(crate) cache_provides: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { cache_provides: true }
    }
}

pub(crate) struct Request {
    registry: Arc<Registry>,
    config: Config,
    #[cfg(feature = "async_tokio")]
    context: Arc<tokio::sync::Mutex<Context>>,
}

impl Request {
    #[inline]
    #[must_use]
    #[cfg(feature = "async_tokio")]
    pub(crate) const fn new(registry: Arc<Registry>, config: Config, context: Arc<tokio::sync::Mutex<Context>>) -> Self {
        Self { registry, config, context }
    }
}

pub(crate) type BoxedCloneInstantiator<DepsErr, FactoryErr> =
    BoxCloneService<Request, Box<dyn Any>, InstantiatorErrorKind<DepsErr, FactoryErr>>;

#[must_use]
pub(crate) fn boxed_instantiator_factory<Inst, Deps>(instantiator: Inst) -> BoxedCloneInstantiator<Deps::Error, Inst::Error>
where
    Inst: Instantiator<Deps>,
    Deps: DependencyResolver,
{
    BoxCloneService::new(service_fn(
        move |Request {
                  registry,
                  config: _config,
                  context,
              }| {
            let mut instantiator = instantiator.clone();

            async move {
                if let Some(dependency) = context.lock().await.get::<Inst::Provides>() {
                    debug!("Found in context");
                    return Ok(Box::new(dependency) as _);
                }
                debug!("Not found in context");

                let dependencies = match Deps::resolve(registry, context).await {
                    Ok(dependencies) => dependencies,
                    Err(err) => return Err(InstantiatorErrorKind::Deps(err)),
                };
                let dependency = match instantiator.instantiate(dependencies).await {
                    Ok(dependency) => dependency,
                    Err(err) => return Err(InstantiatorErrorKind::Factory(err)),
                };

                debug!("Resolved");

                Ok(Box::new(dependency) as _)
            }
            .instrument(debug_span!("instantiator", provides = type_name::<Inst::Provides>()))
        },
    ))
}

#[must_use]
pub(crate) fn boxed_instantiator_cachable_factory<Inst, Deps>(instantiator: Inst) -> BoxedCloneInstantiator<Deps::Error, Inst::Error>
where
    Inst: Instantiator<Deps>,
    Inst::Provides: Clone + Send,
    Deps: DependencyResolver,
{
    BoxCloneService::new(service_fn(move |Request { registry, config, context }| {
        let mut instantiator = instantiator.clone();

        async move {
            if let Some(dependency) = context.lock().await.get::<Inst::Provides>() {
                debug!("Found in context");
                return Ok(Box::new(dependency) as _);
            }
            debug!("Not found in context");

            let dependencies = match Deps::resolve(registry, context.clone()).await {
                Ok(dependencies) => dependencies,
                Err(err) => return Err(InstantiatorErrorKind::Deps(err)),
            };
            let dependency = match instantiator.instantiate(dependencies).await {
                Ok(dependency) => dependency,
                Err(err) => {
                    return Err(InstantiatorErrorKind::Factory(err));
                }
            };

            debug!("Resolved");

            if config.cache_provides {
                context.lock().await.insert(dependency.clone());
                debug!("Cached");
            }

            Ok(Box::new(dependency) as _)
        }
        .instrument(debug_span!("instantiator", provides = type_name::<Inst::Provides>()))
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
        sync::Arc,
    };
    use tracing::debug;
    use tracing_test::traced_test;

    use super::{boxed_instantiator_factory, Config};
    use crate::r#async::{
        context::Context, dependency_resolver::Inject, instantiator::InstantiateErrorKind, registry::Registry, service::Service as _,
    };

    #[derive(Clone, Copy)]
    struct Request(bool);
    #[derive(Clone, Copy)]
    struct Response(bool);

    #[tokio::test]
    #[traced_test]
    #[cfg(feature = "async_tokio")]
    async fn test_boxed_instantiator_factory() {
        let request = Request(true);

        let instantiator_request = boxed_instantiator_factory(move || async move {
            debug!("Call instantiator request");
            Ok::<_, InstantiateErrorKind>(request)
        });
        let mut instantiator_response = boxed_instantiator_factory(|Inject(Request(val))| async move {
            debug!("Call instantiator response");
            Ok::<_, InstantiateErrorKind>(Response(val))
        });

        let mut registry = Registry::default();
        registry.add_instantiator::<Request>(instantiator_request);

        let response = instantiator_response
            .call(super::Request::new(
                Arc::new(registry),
                Config::default(),
                Arc::new(tokio::sync::Mutex::new(Context::default())),
            ))
            .await
            .unwrap();

        assert_eq!(request.0, response.downcast::<Response>().unwrap().0);
    }
}
