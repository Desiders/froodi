use alloc::boxed::Box;
use core::{any::Any, future::Future};
use tracing::debug;

use super::{
    dependency_resolver::DependencyResolver,
    service::{service_fn, BoxCloneService},
    Container,
};
use crate::errors::{InstantiateErrorKind, InstantiatorErrorKind};

pub trait Instantiator<Deps>: Clone + 'static
where
    Deps: DependencyResolver,
{
    type Provides: 'static;
    type Error: Into<InstantiateErrorKind>;

    fn instantiate(&mut self, dependencies: Deps) -> impl Future<Output = Result<Self::Provides, Self::Error>> + Send;
}

/// Config for an instantiator
/// ## Fields
/// - `cache_provides`:
///   If `true`, the instance provided by the instantiator will be cached and reused.
///
///   This does **not** affect the dependencies of the instance.
///   Only the final result is cached if caching is applicable.
#[derive(Clone, Copy)]
pub struct Config {
    pub cache_provides: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { cache_provides: true }
    }
}

pub(crate) type BoxedCloneInstantiator<DepsErr, FactoryErr> =
    BoxCloneService<Container, Box<dyn Any + Send>, InstantiatorErrorKind<DepsErr, FactoryErr>>;

#[must_use]
pub(crate) fn boxed_instantiator_factory<Inst, Deps>(instantiator: Inst) -> BoxedCloneInstantiator<Deps::Error, Inst::Error>
where
    Inst: Instantiator<Deps, Provides: Send> + Send + Sync,
    Deps: DependencyResolver,
{
    BoxCloneService::new(Box::new(service_fn({
        move |container| {
            let mut instantiator = instantiator.clone();

            async move {
                let dependencies = match Deps::resolve(container).await {
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
        }
    })))
}

macro_rules! impl_instantiator {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(non_snake_case)]
        impl<F, Fut, Response, Err, $($ty,)*> Instantiator<($($ty,)*)> for F
        where
            F: FnMut($($ty,)*) -> Fut + Send + Clone + 'static,
            Fut: Future<Output = Result<Response, Err>> + Send,
            Response: 'static,
            Err: Into<InstantiateErrorKind>,
            $( $ty: DependencyResolver + Send, )*
        {
            type Provides = Response;
            type Error = Err;

            fn instantiate(&mut self, ($($ty,)*): ($($ty,)*)) -> impl Future<Output = Result<Self::Provides, Self::Error>> + Send {
                async move { self($($ty,)*).await }
            }
        }
    };
}

all_the_tuples!(impl_instantiator);

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{boxed_instantiator_factory, DependencyResolver, InstantiateErrorKind, Instantiator};
    use crate::{
        r#async::{
            dependency_resolver::{Inject, InjectTransient},
            registry::BoxedInstantiator,
            service::Service as _,
            Container, RegistriesBuilder,
        },
        scope::DefaultScope::*,
    };

    use alloc::{
        format,
        string::{String, ToString as _},
        sync::Arc,
    };
    use core::sync::atomic::{AtomicU8, Ordering};
    use tracing::debug;
    use tracing_test::traced_test;

    struct Request(bool);
    struct Response(bool);

    #[test]
    #[allow(dead_code)]
    fn test_factory_helper() {
        fn resolver<Deps: DependencyResolver, F: Instantiator<Deps>>(_f: F) {}
        fn resolver_with_dep<Deps: DependencyResolver>() {
            resolver(async || Ok::<_, InstantiateErrorKind>(()));
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_boxed_instantiator_factory() {
        let instantiator_request_call_count = Arc::new(AtomicU8::new(0));
        let instantiator_response_call_count = Arc::new(AtomicU8::new(0));

        let instantiator_request = boxed_instantiator_factory({
            let instantiator_request_call_count = instantiator_request_call_count.clone();
            move |()| {
                let instantiator_request_call_count = instantiator_request_call_count.clone();

                async move {
                    instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Call instantiator request");
                    Ok::<_, InstantiateErrorKind>(Request(true))
                }
            }
        });
        let mut instantiator_response = boxed_instantiator_factory({
            let instantiator_response_call_count = instantiator_response_call_count.clone();
            move |InjectTransient(Request(val_1)), InjectTransient(Request(val_2))| {
                let instantiator_response_call_count = instantiator_response_call_count.clone();

                async move {
                    assert_eq!(val_1, val_2);

                    instantiator_response_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Call instantiator response");
                    Ok::<_, InstantiateErrorKind>(Response(val_1))
                }
            }
        });

        let mut registries_builder = RegistriesBuilder::new();
        registries_builder.add_instantiator::<Request>(BoxedInstantiator::Async(instantiator_request), App);

        let container = Container::new(registries_builder);

        let response_1 = instantiator_response.call(container.clone()).await.unwrap();
        let response_2 = instantiator_response.call(container).await.unwrap();

        assert!(response_1.downcast::<Response>().unwrap().0);
        assert!(response_2.downcast::<Response>().unwrap().0);
        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 4);
        assert_eq!(instantiator_response_call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_boxed_instantiator_cached_factory() {
        let instantiator_request_call_count = Arc::new(AtomicU8::new(0));
        let instantiator_response_call_count = Arc::new(AtomicU8::new(0));

        let instantiator_request = boxed_instantiator_factory({
            let instantiator_request_call_count = instantiator_request_call_count.clone();
            move |()| {
                let instantiator_request_call_count = instantiator_request_call_count.clone();

                async move {
                    instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Call instantiator request");
                    Ok::<_, InstantiateErrorKind>(Request(true))
                }
            }
        });
        let mut instantiator_response = boxed_instantiator_factory({
            let instantiator_response_call_count = instantiator_response_call_count.clone();
            move |val_1: Inject<Request>, val_2: Inject<Request>| {
                let instantiator_response_call_count = instantiator_response_call_count.clone();

                async move {
                    assert_eq!(val_1.0 .0, val_2.0 .0);

                    instantiator_response_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Call instantiator response");
                    Ok::<_, InstantiateErrorKind>(Response(val_1.0 .0))
                }
            }
        });

        let mut registries_builder = RegistriesBuilder::new();
        registries_builder.add_instantiator::<Request>(BoxedInstantiator::Async(instantiator_request), App);

        let container = Container::new(registries_builder);

        let response_1 = instantiator_response.call(container.clone()).await.unwrap();
        let response_2 = instantiator_response.call(container.clone()).await.unwrap();
        let response_3 = instantiator_response.call(container).await.unwrap();

        assert!(response_1.downcast::<Response>().unwrap().0);
        assert!(response_2.downcast::<Response>().unwrap().0);
        assert!(response_3.downcast::<Response>().unwrap().0);
        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 1);
        // We don't cache instantiator provides of main factory here, we do it in container
        assert_eq!(instantiator_response_call_count.load(Ordering::SeqCst), 3);
    }
}
