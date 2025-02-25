use alloc::{boxed::Box, rc::Rc};
use core::{
    any::{type_name, Any},
    cell::RefCell,
};
use tracing::{debug, debug_span};

use super::{
    context::Context,
    dependency_resolver::DependencyResolver,
    errors::{InstantiateErrorKind, InstantiatorErrorKind},
    registry::Registry,
    service::{service_fn, BoxCloneService},
};

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
/// - cache_provides:
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
    registry: Rc<Registry>,
    config: Config,
    context: Rc<RefCell<Context>>,
}

impl Request {
    #[inline]
    #[must_use]
    pub(crate) const fn new(registry: Rc<Registry>, config: Config, context: Rc<RefCell<Context>>) -> Self {
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
    BoxCloneService(Box::new(service_fn({
        move |Request {
                  registry,
                  config: _config,
                  context,
              }| {
            let mut instantiator = instantiator.clone();

            if let Some(dependency) = context.borrow().get::<Inst::Provides>() {
                debug!("Found in context");
                return Ok(Box::new(dependency) as _);
            } else {
                debug!("Not found in context");
            };

            let dependencies = match Deps::resolve(registry, context) {
                Ok(dependencies) => dependencies,
                Err(err) => return Err(InstantiatorErrorKind::Deps(err)),
            };
            let dependency = match instantiator.instantiate(dependencies) {
                Ok(dependency) => dependency,
                Err(err) => return Err(InstantiatorErrorKind::Factory(err)),
            };

            debug!("Resolved");

            Ok(Box::new(dependency) as _)
        }
    })))
}

#[must_use]
pub(crate) fn boxed_instantiator_cachable_factory<Inst, Deps>(instantiator: Inst) -> BoxedCloneInstantiator<Deps::Error, Inst::Error>
where
    Inst: Instantiator<Deps>,
    Inst::Provides: Clone,
    Deps: DependencyResolver,
{
    BoxCloneService(Box::new(service_fn({
        move |Request { registry, config, context }| {
            let mut instantiator = instantiator.clone();

            let span = debug_span!("instantiator", provides = type_name::<Inst::Provides>());
            let _guard = span.enter();

            if let Some(dependency) = context.borrow().get::<Inst::Provides>() {
                debug!("Found in context");
                return Ok(Box::new(dependency) as _);
            } else {
                debug!("Not found in context");
            };

            let dependencies = match Deps::resolve(registry, context.clone()) {
                Ok(dependencies) => dependencies,
                Err(err) => return Err(InstantiatorErrorKind::Deps(err)),
            };
            let dependency = match instantiator.instantiate(dependencies) {
                Ok(dependency) => dependency,
                Err(err) => return Err(InstantiatorErrorKind::Factory(err)),
            };

            debug!("Resolved");

            if config.cache_provides {
                context.borrow_mut().insert(dependency.clone());
                debug!("Cached");
            }

            Ok(Box::new(dependency) as _)
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

    use alloc::{
        format,
        rc::Rc,
        string::{String, ToString as _},
    };
    use core::cell::RefCell;
    use tracing::debug;
    use tracing_test::traced_test;

    use super::{boxed_instantiator_factory, Config};
    use crate::{
        context::Context, dependency_resolver::Inject, instantiator::InstantiateErrorKind, registry::Registry, service::Service as _,
    };

    #[derive(Clone, Copy)]
    struct Request(bool);
    struct Response(bool);

    #[test]
    #[traced_test]
    fn test_boxed_instantiator_factory() {
        let request = Request(true);

        let instantiator_request = boxed_instantiator_factory(move || {
            debug!("Call instantiator request");
            Ok::<_, InstantiateErrorKind>(request)
        });
        let mut instantiator_response = boxed_instantiator_factory(|Inject(Request(val))| {
            debug!("Call instantiator response");
            Ok::<_, InstantiateErrorKind>(Response(val))
        });

        let mut registry = Registry::default();
        registry.add_instantiator::<Request>(instantiator_request);

        let response = instantiator_response
            .call(super::Request::new(
                Rc::new(registry),
                Config::default(),
                Rc::new(RefCell::new(Context::default())),
            ))
            .unwrap();

        assert_eq!(request.0, response.downcast::<Response>().unwrap().0);
    }
}
