use alloc::{boxed::Box, collections::btree_set::BTreeSet};
use core::{any::Any, future::Future};
use tracing::debug;

use super::{
    service::{service_fn, BoxCloneService},
    Container,
};
use crate::{
    dependency::Dependency,
    dependency_resolver::DependencyResolver,
    errors::{InstantiateErrorKind, InstantiatorErrorKind},
    utils::thread_safety::{SendSafety, SyncSafety},
    ResolveErrorKind,
};

pub trait Instantiator<Deps>: Clone + 'static
where
    Deps: DependencyResolver,
{
    type Provides: 'static;
    type Error: Into<InstantiateErrorKind>;

    fn instantiate(&mut self, dependencies: Deps) -> impl Future<Output = Result<Self::Provides, Self::Error>> + SendSafety;

    fn dependencies() -> BTreeSet<Dependency>;
}

pub(crate) type BoxedCloneInstantiator<DepsErr, FactoryErr> =
    BoxCloneService<Container, Box<dyn Any>, InstantiatorErrorKind<DepsErr, FactoryErr>>;

#[must_use]
pub(crate) fn boxed_instantiator<Inst, Deps>(instantiator: Inst) -> BoxedCloneInstantiator<Deps::Error, Inst::Error>
where
    Inst: Instantiator<Deps> + SendSafety + SyncSafety,
    Deps: DependencyResolver,
{
    BoxCloneService::new(Box::new(service_fn({
        move |container| {
            let mut instantiator = instantiator.clone();

            async move {
                let dependencies = match Deps::resolve_async(&container).await {
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

#[must_use]
pub(crate) fn boxed_container_instantiator() -> BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind> {
    BoxCloneService::new(Box::new(service_fn(async move |container| Ok(Box::new(container) as _))))
}

macro_rules! impl_instantiator {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(non_snake_case)]
        impl<F, Fut, Response, Err, $($ty,)*> Instantiator<($($ty,)*)> for F
        where
            F: FnMut($($ty,)*) -> Fut + SendSafety + Clone + 'static,
            Fut: Future<Output = Result<Response, Err>> + SendSafety,
            Response: 'static,
            Err: Into<InstantiateErrorKind>,
            $( $ty: DependencyResolver + SendSafety + 'static, )*
        {
            type Provides = Response;
            type Error = Err;

            fn instantiate(&mut self, ($($ty,)*): ($($ty,)*)) -> impl Future<Output = Result<Self::Provides, Self::Error>> + SendSafety  {
                async move { self($($ty,)*).await }
            }

            #[inline]
            fn dependencies() -> BTreeSet<Dependency> {
                BTreeSet::from_iter([
                    $(
                        Dependency {
                            type_info: $ty::type_info(),
                        }
                    ),*
                ])
            }
        }
    };
}

all_the_tuples!(impl_instantiator);

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{boxed_instantiator, DependencyResolver, InstantiateErrorKind, Instantiator};
    use crate::{
        async_impl::{service::Service as _, Container},
        async_registry,
        scope::DefaultScope::*,
        utils::thread_safety::RcThreadSafety,
        Inject, InjectTransient,
    };

    use alloc::{
        format,
        string::{String, ToString as _},
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
        let instantiator_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let instantiator_response_call_count = RcThreadSafety::new(AtomicU8::new(0));

        let mut instantiator_response = boxed_instantiator({
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

        let container = Container::new(async_registry! {
            scope(App) [
                provide({
                    let instantiator_request_call_count = instantiator_request_call_count.clone();
                    move |()| {
                        let instantiator_request_call_count = instantiator_request_call_count.clone();

                        async move {
                            instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                            debug!("Call instantiator request");
                            Ok::<_, InstantiateErrorKind>(Request(true))
                        }
                    }
                }),
            ]
        });

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
        let instantiator_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let instantiator_response_call_count = RcThreadSafety::new(AtomicU8::new(0));

        let mut instantiator_response = boxed_instantiator({
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

        let container = Container::new(async_registry! {
            scope(App) [
                provide({
                    let instantiator_request_call_count = instantiator_request_call_count.clone();
                    move |()| {
                        let instantiator_request_call_count = instantiator_request_call_count.clone();

                        async move {
                            instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                            debug!("Call instantiator request");
                            Ok::<_, InstantiateErrorKind>(Request(true))
                        }
                    }
                }),
            ]
        });

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
