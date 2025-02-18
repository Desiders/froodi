use alloc::{boxed::Box, rc::Rc};
use core::{any::type_name, cell::RefCell};
use tracing::{debug, error, instrument, warn};

use crate::{
    context::Context,
    instantiator::{InstantiateErrorKind, InstantiatorErrorKind, Request},
    registry::Registry,
    service::Service as _,
};

#[derive(Debug)]
pub(crate) enum ResolveErrorKind {
    NoFactory,
    Factory(InstantiatorErrorKind<Box<ResolveErrorKind>, InstantiateErrorKind>),
}

pub(crate) trait DependencyResolver: Sized {
    type Error: Into<ResolveErrorKind>;

    fn resolve(registry: Rc<Registry>, context: Rc<RefCell<Context>>) -> Result<Self, Self::Error>;
}

pub(crate) struct Inject<Dep, const CACHABLE: bool = true>(pub(crate) Dep);

impl<Dep: Clone + 'static> DependencyResolver for Inject<Dep, true> {
    type Error = ResolveErrorKind;

    #[instrument(skip_all, fields(dependency = type_name::<Dep>()))]
    fn resolve(registry: Rc<Registry>, context: Rc<RefCell<Context>>) -> Result<Self, Self::Error> {
        if let Some(dependency) = context.borrow().get::<Dep>() {
            debug!("Dependency found in context");
            return Ok(Self(dependency));
        } else {
            debug!("Dependency not found in context");
        }

        let Some((mut instantiator, config)) = registry.get_instantiator::<Dep>() else {
            debug!("Instantiator not found in registry");
            return Err(ResolveErrorKind::NoFactory);
        };
        let cache_provides = config.cache_provides;

        let dependency = match instantiator.call(Request::new(registry, context.clone())) {
            Ok(dependency) => match dependency.downcast::<Dep>() {
                Ok(dependency) => *dependency,
                Err(incorrect_type) => {
                    error!("Incorrect factory provides type: {incorrect_type:#?}");
                    unreachable!("Incorrect factory provides type: {incorrect_type:#?}");
                }
            },
            Err(InstantiatorErrorKind::Deps(err)) => {
                error!("Resolve error kind: {err:#?}");
                return Err(ResolveErrorKind::Factory(InstantiatorErrorKind::Deps(
                    Box::new(err),
                )));
            }
            Err(InstantiatorErrorKind::Factory(err)) => {
                error!("Instantiate error kind: {err:#?}");
                return Err(ResolveErrorKind::Factory(InstantiatorErrorKind::Factory(
                    err,
                )));
            }
        };

        debug!("Dependency resolved");

        if cache_provides {
            context.borrow_mut().insert(dependency.clone());

            debug!("Dependency cached");
        }

        Ok(Self(dependency))
    }
}

impl<Dep: 'static> DependencyResolver for Inject<Dep, false> {
    type Error = ResolveErrorKind;

    #[instrument(skip_all, fields(dependency = type_name::<Dep>()))]
    fn resolve(registry: Rc<Registry>, context: Rc<RefCell<Context>>) -> Result<Self, Self::Error> {
        if let Some(dependency) = context.borrow().get::<Dep>() {
            debug!("Dependency found in context");
            return Ok(Self(dependency));
        } else {
            debug!("Dependency not found in context");
        }

        let Some((mut instantiator, _config)) = registry.get_instantiator::<Dep>() else {
            debug!("Instantiator not found in registry");
            return Err(ResolveErrorKind::NoFactory);
        };

        let dependency = match instantiator.call(Request::new(registry, context)) {
            Ok(dependency) => match dependency.downcast::<Dep>() {
                Ok(dependency) => *dependency,
                Err(incorrect_type) => {
                    error!("Incorrect factory provides type: {incorrect_type:#?}");
                    unreachable!("Incorrect factory provides type: {incorrect_type:#?}");
                }
            },
            Err(InstantiatorErrorKind::Deps(err)) => {
                error!("Resolve error kind: {err:#?}");
                return Err(ResolveErrorKind::Factory(InstantiatorErrorKind::Deps(
                    Box::new(err),
                )));
            }
            Err(InstantiatorErrorKind::Factory(err)) => {
                error!("Instantiate error kind: {err:#?}");
                return Err(ResolveErrorKind::Factory(InstantiatorErrorKind::Factory(
                    err,
                )));
            }
        };

        debug!("Dependency resolved");

        Ok(Self(dependency))
    }
}

macro_rules! impl_dependency_resolver {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(non_snake_case)]
        impl<$($ty,)*> DependencyResolver for ($($ty,)*)
        where
            $( $ty: DependencyResolver, )*
        {
            type Error = ResolveErrorKind;

            #[inline]
            #[allow(unused_variables)]
            fn resolve(registry: Rc<Registry>, context: Rc<RefCell<Context>>) -> Result<Self, Self::Error> {
                Ok(($($ty::resolve(registry.clone(), context.clone()).map_err(Into::into)?,)*))
            }
        }
    };
}

all_the_tuples!(impl_dependency_resolver);
