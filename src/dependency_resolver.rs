use alloc::{boxed::Box, rc::Rc};
use core::any::type_name;
use tracing::{debug, error, instrument};

use crate::{
    context::Context,
    instantiator::{InstantiateErrorKind, InstantiatorErrorKind, RequestSync},
    registry::Registry,
    service::base::Service as _,
};

#[derive(Debug)]
pub(crate) enum ResolveErrorKind {
    NoFactory,
    Factory(InstantiatorErrorKind<Box<ResolveErrorKind>, InstantiateErrorKind>),
}

pub(crate) trait DependencyResolverSync: Sized {
    type Error;

    fn resolve(registry: Rc<Registry>, context: Rc<Context>) -> Result<Self, Self::Error>;
}

pub(crate) struct Inject<Dep>(pub(crate) Dep);

impl<Dep: 'static> DependencyResolverSync for Inject<Dep> {
    type Error = ResolveErrorKind;

    #[instrument(skip_all, fields(dependency = type_name::<Dep>()))]
    fn resolve(registry: Rc<Registry>, context: Rc<Context>) -> Result<Self, Self::Error> {
        if let Some(dependency) = context.get::<Dep>() {
            debug!("Dependency found in context");
            return Ok(Self(dependency));
        } else {
            debug!("Dependency not found in context");
        }

        let Some(mut factory) = registry.get_instantiator::<Dep>() else {
            debug!("Instantiator not found in registry");
            return Err(ResolveErrorKind::NoFactory);
        };

        let dependency = match factory.call(RequestSync::new(registry, context)) {
            Ok(dependency) => *match dependency.downcast::<Dep>() {
                Ok(dependency) => dependency,
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

macro_rules! impl_dependency_resolver_sync {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(non_snake_case)]
        impl<$($ty,)*> DependencyResolverSync for ($($ty,)*)
        where
            $( $ty: DependencyResolverSync<Error: Into<ResolveErrorKind>>, )*
        {
            type Error = ResolveErrorKind;

            #[inline]
            #[allow(unused_variables)]
            fn resolve(registry: Rc<Registry>, context: Rc<Context>) -> Result<Self, Self::Error> {
                Ok(($($ty::resolve(registry.clone(), context.clone()).map_err(Into::into)?,)*))
            }
        }
    };
}

all_the_tuples!(impl_dependency_resolver_sync);
