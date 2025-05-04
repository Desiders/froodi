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

        let Some((mut instantiator, config)) = registry.get_instantiator_with_config::<Dep>() else {
            let err = ResolveErrorKind::NoInstantiator;
            warn!("{}", err);
            return Err(err);
        };

        match instantiator.call(Request::new(registry, context.clone())) {
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

        let Some(mut instantiator) = registry.get_instantiator::<Dep>() else {
            let err = ResolveErrorKind::NoInstantiator;
            warn!("{}", err);
            return Err(err);
        };

        match instantiator.call(Request::new(registry, context.clone())) {
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

#[cfg(test)]
mod tests {
    extern crate std;

    use alloc::{
        format,
        rc::Rc,
        string::{String, ToString as _},
    };
    use core::{
        cell::RefCell,
        sync::atomic::{AtomicU8, Ordering},
    };
    use tracing::debug;
    use tracing_test::traced_test;

    use super::{Context, DependencyResolver, Inject, InjectTransient, Registry};
    use crate::{errors::InstantiateErrorKind, instantiator::boxed_instantiator_factory};

    struct Request;

    #[test]
    fn test_dependency_resolver_impls() {
        fn resolver<T: DependencyResolver>() {}
        fn resolver_with_dep<Dep: 'static>() {
            resolver::<Inject<Dep>>();
            resolver::<InjectTransient<Dep>>();

            resolver::<(Inject<Dep>, InjectTransient<Dep>)>();
        }
    }

    #[test]
    #[traced_test]
    fn test_scoped_resolve() {
        let instantiator_request_call_count = Rc::new(AtomicU8::new(0));

        let mut registry = Registry::default();
        registry.add_instantiator::<Request>(boxed_instantiator_factory({
            let instantiator_request_call_count = instantiator_request_call_count.clone();
            move || {
                instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                debug!("Call instantiator request");
                Ok::<_, InstantiateErrorKind>(Request)
            }
        }));

        let registry = Rc::new(registry);
        let context = Rc::new(RefCell::new(Context::default()));

        let request_1 = Inject::<Request>::resolve(registry.clone(), context.clone()).unwrap();
        let request_2 = Inject::<Request>::resolve(registry, context).unwrap();

        assert!(Rc::ptr_eq(&request_1.0, &request_2.0));
        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    #[traced_test]
    fn test_transient_resolve() {
        let instantiator_request_call_count = Rc::new(AtomicU8::new(0));

        let mut registry = Registry::default();
        registry.add_instantiator::<Request>(boxed_instantiator_factory({
            let instantiator_request_call_count = instantiator_request_call_count.clone();
            move || {
                instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                debug!("Call instantiator request");
                Ok::<_, InstantiateErrorKind>(Request)
            }
        }));

        let registry = Rc::new(registry);
        let context = Rc::new(RefCell::new(Context::default()));

        InjectTransient::<Request>::resolve(registry.clone(), context.clone()).unwrap();
        InjectTransient::<Request>::resolve(registry, context).unwrap();

        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 2);
    }
}
