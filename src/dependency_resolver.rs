use alloc::{boxed::Box, rc::Rc};
use core::{any::type_name, cell::RefCell};
use tracing::{debug_span, error, warn};

use super::{
    context::Context,
    errors::{InstantiatorErrorKind, ResolveErrorKind},
    instantiator::Request,
    registry::Registry,
    service::Service as _,
};

pub(crate) trait DependencyResolver: Sized {
    type Error: Into<ResolveErrorKind>;

    fn resolve(registry: Rc<Registry>, context: Rc<RefCell<Context>>) -> Result<Self, Self::Error>;
}

pub(crate) struct Inject<Dep>(pub(crate) Dep);

impl<Dep: 'static> DependencyResolver for Inject<Dep> {
    type Error = ResolveErrorKind;

    fn resolve(registry: Rc<Registry>, context: Rc<RefCell<Context>>) -> Result<Self, Self::Error> {
        let span = debug_span!("resolve", dependency = type_name::<Dep>());
        let _guard = span.enter();

        let Some((mut instantiator, config)) = registry.get_instantiator::<Dep>() else {
            warn!("Instantiator not found in registry");
            return Err(ResolveErrorKind::NoFactory);
        };

        let dependency = match instantiator.call(Request::new(registry, config, context)) {
            Ok(dependency) => match dependency.downcast::<Dep>() {
                Ok(dependency) => *dependency,
                Err(incorrect_type) => {
                    error!("Incorrect instantiator provides type: {incorrect_type:#?}");
                    unreachable!("Incorrect instantiator provides type: {incorrect_type:#?}");
                }
            },
            Err(InstantiatorErrorKind::Deps(err)) => {
                error!(%err);
                return Err(ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(Box::new(err))));
            }
            Err(InstantiatorErrorKind::Factory(err)) => {
                error!(%err);
                return Err(ResolveErrorKind::Instantiator(InstantiatorErrorKind::Factory(err)));
            }
        };

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
