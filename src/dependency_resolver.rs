use alloc::{boxed::Box, rc::Rc};
use core::{
    any::{type_name, TypeId},
    cell::RefCell,
};
use tracing::{debug, debug_span, error, warn};

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

pub(crate) struct Inject<Dep>(pub(crate) Rc<Dep>);

impl<Dep: 'static> DependencyResolver for Inject<Dep> {
    type Error = ResolveErrorKind;

    fn resolve(registry: Rc<Registry>, context: Rc<RefCell<Context>>) -> Result<Self, Self::Error> {
        let span = debug_span!("resolve", dependency = type_name::<Dep>());
        let _guard = span.enter();

        if let Some(dependency) = context.borrow().get::<Dep>() {
            debug!("Found in context");
            return Ok(Self(dependency));
        }
        debug!("Not found in context");

        let Some((mut instantiator, config)) = registry.get_instantiator::<Dep>() else {
            let err = ResolveErrorKind::NoInstantiator;
            warn!("{}", err);
            return Err(err);
        };

        match instantiator.call(Request::new(registry, config, context.clone())) {
            Ok(dependency) => match dependency.downcast::<Dep>() {
                Ok(dependency) => {
                    let dependency = Rc::new(*dependency);
                    if config.cache_provides {
                        context.borrow_mut().insert_rc(dependency.clone());
                        debug!("Cached");
                    }
                    Ok(Self(dependency))
                }
                Err(incorrect_type) => {
                    let err = ResolveErrorKind::IncorrectType {
                        expected: TypeId::of::<Dep>(),
                        actual: incorrect_type.type_id(),
                    };
                    error!("{}", err);
                    Err(err)
                }
            },
            Err(InstantiatorErrorKind::Deps(err)) => {
                error!("{}", err);
                Err(ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(Box::new(err))))
            }
            Err(InstantiatorErrorKind::Factory(err)) => {
                error!("{}", err);
                Err(ResolveErrorKind::Instantiator(InstantiatorErrorKind::Factory(err)))
            }
        }
    }
}

pub(crate) struct InjectTransient<Dep>(pub(crate) Dep);

impl<Dep: 'static> DependencyResolver for InjectTransient<Dep> {
    type Error = ResolveErrorKind;

    fn resolve(registry: Rc<Registry>, context: Rc<RefCell<Context>>) -> Result<Self, Self::Error> {
        let span = debug_span!("resolve", dependency = type_name::<Dep>());
        let _guard = span.enter();

        let Some((mut instantiator, config)) = registry.get_instantiator::<Dep>() else {
            let err = ResolveErrorKind::NoInstantiator;
            warn!("{}", err);
            return Err(err);
        };

        match instantiator.call(Request::new(registry, config, context.clone())) {
            Ok(dependency) => match dependency.downcast::<Dep>() {
                Ok(dependency) => Ok(Self(*dependency as _)),
                Err(incorrect_type) => {
                    let err = ResolveErrorKind::IncorrectType {
                        expected: TypeId::of::<Dep>(),
                        actual: incorrect_type.type_id(),
                    };
                    error!("{}", err);
                    Err(err)
                }
            },
            Err(InstantiatorErrorKind::Deps(err)) => {
                error!("{}", err);
                Err(ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(Box::new(err))))
            }
            Err(InstantiatorErrorKind::Factory(err)) => {
                error!("{}", err);
                Err(ResolveErrorKind::Instantiator(InstantiatorErrorKind::Factory(err)))
            }
        }
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
