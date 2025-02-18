use alloc::{boxed::Box, rc::Rc};
use core::{any::Any, cell::RefCell};

use crate::{
    context::Context,
    dependency_resolver::DependencyResolverSync,
    registry::Registry,
    service::{boxed_clone::BoxCloneServiceSync, fn_service::FnServiceSync},
};

#[derive(Debug)]
pub(crate) enum InstantiateErrorKind {}

pub(crate) trait InstantiatorSync<Deps> {
    type Provides;
    type Error;

    fn instantiate(&mut self, dependencies: Deps) -> Result<Self::Provides, Self::Error>;
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
    registry: Rc<Registry>,
    context: Rc<RefCell<Context>>,
}

impl Request {
    #[inline]
    #[must_use]
    pub(crate) const fn new(registry: Rc<Registry>, context: Rc<RefCell<Context>>) -> Self {
        Self { registry, context }
    }
}

pub(crate) type BoxedCloneInstantiatorSync<DepsErr, FactoryErr> =
    BoxCloneServiceSync<Request, Box<dyn Any>, InstantiatorErrorKind<DepsErr, FactoryErr>>;

#[derive(Debug)]
pub(crate) enum InstantiatorErrorKind<DepsErr, FactoryErr> {
    Deps(DepsErr),
    Factory(FactoryErr),
}

#[must_use]
pub(crate) fn instantiator_sync<Instantiator, Deps>(
    instantiator: Instantiator,
) -> BoxedCloneInstantiatorSync<Deps::Error, Instantiator::Error>
where
    Instantiator: InstantiatorSync<Deps> + Clone + 'static,
    Instantiator::Provides: 'static,
    Deps: DependencyResolverSync,
{
    BoxCloneServiceSync(Box::new(FnServiceSync({
        let mut instantiator = instantiator.clone();

        move |Request { registry, context }| {
            let dependencies = match Deps::resolve(registry, context) {
                Ok(dependencies) => dependencies,
                Err(err) => return Err(InstantiatorErrorKind::Deps(err)),
            };
            let dependency = match instantiator.instantiate(dependencies) {
                Ok(dependency) => dependency,
                Err(err) => return Err(InstantiatorErrorKind::Factory(err)),
            };

            Ok(Box::new(dependency) as _)
        }
    })))
}

macro_rules! impl_instantiator_sync {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(non_snake_case)]
        impl<F, Response, Err, $($ty,)*> InstantiatorSync<($($ty,)*)> for F
        where
            F: Fn($($ty,)*) -> Result<Response, Err>,
            Err: Into<InstantiateErrorKind>,
            $( $ty: DependencyResolverSync, )*
        {
            type Provides = Response;
            type Error = Err;

            fn instantiate(&mut self, ($($ty,)*): ($($ty,)*)) -> Result<Self::Provides, Self::Error> {
                self($($ty,)*)
            }
        }
    };
}

all_the_tuples!(impl_instantiator_sync);

#[cfg(test)]
mod tests {
    extern crate std;

    use alloc::{
        format,
        rc::Rc,
        string::{String, ToString as _},
    };
    use core::cell::RefCell;
    use tracing::debug;
    use tracing_test::traced_test;

    use super::instantiator_sync;
    use crate::{
        context::Context, dependency_resolver::Inject, instantiator::InstantiateErrorKind,
        registry::Registry, service::base::Service as _,
    };

    #[derive(Clone, Copy)]
    struct Request(bool);
    struct Response(bool);

    #[test]
    #[traced_test]
    fn test_instantiator_sync() {
        let request = Request(true);

        let instantiator_request = instantiator_sync(move || {
            debug!("Call instantiator request");
            Ok::<_, InstantiateErrorKind>(request)
        });
        let mut instantiator_response =
            instantiator_sync(|Inject(Request(val)): Inject<_, true>| {
                debug!("Call instantiator response");
                Ok::<_, InstantiateErrorKind>(Response(val))
            });

        let mut registry = Registry::default();
        registry.add_instantiator::<Request>(instantiator_request);

        let response = instantiator_response
            .call(super::Request::new(
                Rc::new(registry),
                Rc::new(RefCell::new(Context::default())),
            ))
            .unwrap();

        assert_eq!(request.0, response.downcast::<Response>().unwrap().0);
    }
}
