use alloc::{boxed::Box, sync::Arc};
use core::any::Any;
use tracing::debug;

use super::{
    context::Context,
    dependency_resolver::DependencyResolver,
    errors::{InstantiateErrorKind, InstantiatorErrorKind},
    service::{service_fn, BoxCloneService},
};
use crate::registry::Registry;

pub(crate) trait Instantiator<Deps>: Clone + 'static
where
    Deps: DependencyResolver,
{
    type Provides: 'static;
    type Error: Into<InstantiateErrorKind>;

    fn instantiate(&mut self, dependencies: Deps) -> Result<Self::Provides, Self::Error>;
}

/// Config for an instantiator
/// ## Fields
/// - `cache_provides`:
///   If `true`, the instance provided by the instantiator will be cached and reused.
///
///   This does **not** affect the dependencies of the instance.
///   Only the final result is cached if caching is applicable.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Config {
    pub cache_provides: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { cache_provides: true }
    }
}

pub(crate) struct Request {
    registry: Arc<Registry>,
    context: Context,
}

impl Request {
    #[inline]
    #[must_use]
    pub(crate) const fn new(registry: Arc<Registry>, context: Context) -> Self {
        Self { registry, context }
    }
}

pub(crate) type BoxedCloneInstantiator<DepsErr, FactoryErr> =
    BoxCloneService<Request, (Box<dyn Any>, Context), InstantiatorErrorKind<DepsErr, FactoryErr>>;

#[must_use]
pub(crate) fn boxed_instantiator_factory<Inst, Deps>(instantiator: Inst) -> BoxedCloneInstantiator<Deps::Error, Inst::Error>
where
    Inst: Instantiator<Deps> + Send + Sync,
    Deps: DependencyResolver,
{
    BoxCloneService(Box::new(service_fn({
        move |Request { registry, context }| {
            let (dependencies, context) = match Deps::resolve(registry, context) {
                Ok(dependencies) => dependencies,
                Err(err) => return Err(InstantiatorErrorKind::Deps(err)),
            };
            let dependency = match instantiator.clone().instantiate(dependencies) {
                Ok(dependency) => dependency,
                Err(err) => return Err(InstantiatorErrorKind::Factory(err)),
            };

            debug!("Resolved");

            Ok((Box::new(dependency) as _, context))
        }
    })))
}

macro_rules! impl_instantiator {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(non_snake_case)]
        impl<F, Response, Err, $($ty,)*> Instantiator<($($ty,)*)> for F
        where
            F: FnMut($($ty,)*) -> Result<Response, Err> + Clone + 'static,
            Response: 'static,
            Err: Into<InstantiateErrorKind>,
            $( $ty: DependencyResolver, )*
        {
            type Provides = Response;
            type Error = Err;

            fn instantiate(&mut self, ($($ty,)*): ($($ty,)*)) -> Result<Self::Provides, Self::Error> {
                self($($ty,)*)
            }
        }
    };
}

all_the_tuples!(impl_instantiator);

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{boxed_instantiator_factory, Context, DependencyResolver, InstantiateErrorKind, Instantiator};
    use crate::{
        dependency_resolver::{Inject, InjectTransient},
        scope::DefaultScope::*,
        service::Service as _,
        RegistriesBuilder,
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
            resolver(|| Ok::<_, InstantiateErrorKind>(()));
        }
    }

    #[test]
    #[traced_test]
    fn test_boxed_instantiator_factory() {
        let instantiator_request_call_count = Arc::new(AtomicU8::new(0));
        let instantiator_response_call_count = Arc::new(AtomicU8::new(0));

        let instantiator_request = boxed_instantiator_factory({
            let instantiator_request_call_count = instantiator_request_call_count.clone();
            move || {
                instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                debug!("Call instantiator request");
                Ok::<_, InstantiateErrorKind>(Request(true))
            }
        });
        let mut instantiator_response = boxed_instantiator_factory({
            let instantiator_response_call_count = instantiator_response_call_count.clone();
            move |InjectTransient(Request(val_1)), InjectTransient(Request(val_2))| {
                assert_eq!(val_1, val_2);

                instantiator_response_call_count.fetch_add(1, Ordering::SeqCst);

                debug!("Call instantiator response");
                Ok::<_, InstantiateErrorKind>(Response(val_1))
            }
        });

        let mut registries_builder = RegistriesBuilder::new();
        registries_builder.add_instantiator::<Request>(instantiator_request, App);

        let mut registries = registries_builder.build().into_iter();
        let registry = if let Some(root_registry) = registries.next() {
            Arc::new(root_registry)
        } else {
            panic!("registries len (is 0) should be >= 1");
        };
        let context = Context::new();

        let (response_1, context) = instantiator_response.call(super::Request::new(registry.clone(), context)).unwrap();
        let (response_2, _) = instantiator_response.call(super::Request::new(registry, context)).unwrap();

        assert!(response_1.downcast::<Response>().unwrap().0);
        assert!(response_2.downcast::<Response>().unwrap().0);
        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 4);
        assert_eq!(instantiator_response_call_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    #[traced_test]
    fn test_boxed_instantiator_cached_factory() {
        let instantiator_request_call_count = Arc::new(AtomicU8::new(0));
        let instantiator_response_call_count = Arc::new(AtomicU8::new(0));

        let instantiator_request = boxed_instantiator_factory({
            let instantiator_request_call_count = instantiator_request_call_count.clone();
            move || {
                instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                debug!("Call instantiator request");
                Ok::<_, InstantiateErrorKind>(Request(true))
            }
        });
        let mut instantiator_response = boxed_instantiator_factory({
            let instantiator_response_call_count = instantiator_response_call_count.clone();
            move |val_1: Inject<Request>, val_2: Inject<Request>| {
                assert_eq!(val_1.0 .0, val_2.0 .0);

                instantiator_response_call_count.fetch_add(1, Ordering::SeqCst);

                debug!("Call instantiator response");
                Ok::<_, InstantiateErrorKind>(Response(val_1.0 .0))
            }
        });

        let mut registries_builder = RegistriesBuilder::new();
        registries_builder.add_instantiator::<Request>(instantiator_request, App);

        let mut registries = registries_builder.build().into_iter();
        let registry = if let Some(root_registry) = registries.next() {
            Arc::new(root_registry)
        } else {
            panic!("registries len (is 0) should be >= 1");
        };
        let context = Context::new();

        let (response_1, context) = instantiator_response.call(super::Request::new(registry.clone(), context)).unwrap();
        let (response_2, context) = instantiator_response.call(super::Request::new(registry.clone(), context)).unwrap();
        let (response_3, _) = instantiator_response.call(super::Request::new(registry, context)).unwrap();

        assert!(response_1.downcast::<Response>().unwrap().0);
        assert!(response_2.downcast::<Response>().unwrap().0);
        assert!(response_3.downcast::<Response>().unwrap().0);
        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 1);
        // We don't cache instantiator provides of main factory here, we do it in container
        assert_eq!(instantiator_response_call_count.load(Ordering::SeqCst), 3);
    }
}
