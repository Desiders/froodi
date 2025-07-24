use alloc::{boxed::Box, collections::vec_deque::VecDeque, rc::Rc};
use core::any::{type_name, Any, TypeId};
use tracing::{debug, debug_span, error, warn};

use crate::registry::{InstantiatorInnerData, Registry};

use super::{
    context::Context,
    errors::{InstantiatorErrorKind, ResolveErrorKind},
    instantiator::Request,
    service::Service as _,
};

pub(crate) trait DependencyResolver: Sized {
    type Error: Into<ResolveErrorKind>;

    fn resolve(registry: Rc<Registry>, context: Context) -> Result<(Self, Context), Self::Error>;
}

pub(crate) struct Inject<Dep>(pub(crate) Rc<Dep>);

impl<Dep: 'static> DependencyResolver for Inject<Dep> {
    type Error = ResolveErrorKind;

    fn resolve(registry: Rc<Registry>, context: Context) -> Result<(Self, Context), Self::Error> {
        let span = debug_span!("resolve", dependency = type_name::<Dep>());
        let _guard = span.enter();

        let type_id = TypeId::of::<Dep>();

        if let Some(dependency) = context.get(&type_id) {
            debug!("Found in context");
            return Ok((Self(dependency), context));
        }
        debug!("Not found in context");

        let Some(InstantiatorInnerData {
            mut instantiator,
            finalizer,
            config,
        }) = registry.get_instantiator_data(&type_id)
        else {
            let err = ResolveErrorKind::NoInstantiator;
            warn!("{}", err);
            return Err(err);
        };

        match instantiator.call(Request::new(registry, context)) {
            Ok((dependency, mut context)) => match dependency.downcast::<Dep>() {
                Ok(dependency) => {
                    let dependency = Rc::new(*dependency);
                    if config.cache_provides {
                        context.insert_rc(dependency.clone());
                        debug!("Cached");
                    }
                    if finalizer.is_some() {
                        context.push_resolved(Resolved {
                            type_id,
                            dependency: dependency.clone(),
                        });
                        debug!("Pushed to resolved set");
                    }
                    Ok((Self(dependency), context))
                }
                Err(incorrect_type) => {
                    let err = ResolveErrorKind::IncorrectType {
                        expected: type_id,
                        actual: (*incorrect_type).type_id(),
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

    fn resolve(registry: Rc<Registry>, context: Context) -> Result<(Self, Context), Self::Error> {
        let span = debug_span!("resolve", dependency = type_name::<Dep>());
        let _guard = span.enter();

        let type_id = TypeId::of::<Dep>();

        let Some(mut instantiator) = registry.get_instantiator(&type_id) else {
            let err = ResolveErrorKind::NoInstantiator;
            warn!("{}", err);
            return Err(err);
        };

        match instantiator.call(Request::new(registry, context)) {
            Ok((dependency, context)) => match dependency.downcast::<Dep>() {
                Ok(dependency) => Ok((Self(*dependency as _), context)),
                Err(incorrect_type) => {
                    let err = ResolveErrorKind::IncorrectType {
                        expected: type_id,
                        actual: (*incorrect_type).type_id(),
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
        #[allow(non_snake_case, unused_mut)]
        impl<$($ty,)*> DependencyResolver for ($($ty,)*)
        where
            $( $ty: DependencyResolver, )*
        {
            type Error = ResolveErrorKind;

            #[inline]
            #[allow(unused_variables)]
            fn resolve(registry: Rc<Registry>, context: Context) -> Result<(Self, Context), Self::Error> {
                let mut context = context;
                Ok((
                    (
                        $(
                            {
                                let ($ty, updated_context) = $ty::resolve(registry.clone(), context).map_err(Into::into)?;
                                context = updated_context;
                                $ty
                            }
                        ,)*
                    ),
                    context,
                ))
            }
        }
    };
}

all_the_tuples!(impl_dependency_resolver);

#[derive(Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub(crate) struct Resolved {
    pub(crate) type_id: TypeId,
    pub(crate) dependency: Rc<dyn Any>,
}

#[derive(Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub(crate) struct ResolvedSet(pub(crate) VecDeque<Resolved>);

impl ResolvedSet {
    pub(crate) const fn new() -> Self {
        Self(VecDeque::new())
    }

    pub(crate) fn push(&mut self, resolved: Resolved) {
        self.0.push_back(resolved);
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{Context, DependencyResolver, Inject, InjectTransient};
    use crate::{errors::InstantiateErrorKind, scope::DefaultScope, RegistriesBuilder};

    use alloc::{
        format,
        rc::Rc,
        string::{String, ToString as _},
    };
    use core::sync::atomic::{AtomicU8, Ordering};
    use tracing::debug;
    use tracing_test::traced_test;

    struct Request;

    #[test]
    #[allow(dead_code)]
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

        let registries_builder = RegistriesBuilder::new().provide(
            {
                let instantiator_request_call_count = instantiator_request_call_count.clone();
                move || {
                    instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Call instantiator request");
                    Ok::<_, InstantiateErrorKind>(Request)
                }
            },
            DefaultScope::App,
        );

        let mut registries = registries_builder.build().into_iter();
        let registry = if let Some(root_registry) = registries.next() {
            Rc::new(root_registry)
        } else {
            panic!("registries len (is 0) should be >= 1");
        };

        let context = Context::new();

        let (request_1, context) = Inject::<Request>::resolve(registry.clone(), context).unwrap();
        let (request_2, _) = Inject::<Request>::resolve(registry, context).unwrap();

        assert!(Rc::ptr_eq(&request_1.0, &request_2.0));
        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    #[traced_test]
    fn test_transient_resolve() {
        let instantiator_request_call_count = Rc::new(AtomicU8::new(0));

        let registries_builder = RegistriesBuilder::new().provide(
            {
                let instantiator_request_call_count = instantiator_request_call_count.clone();
                move || {
                    instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Call instantiator request");
                    Ok::<_, InstantiateErrorKind>(Request)
                }
            },
            DefaultScope::App,
        );

        let mut registries = registries_builder.build().into_iter();
        let registry = if let Some(root_registry) = registries.next() {
            Rc::new(root_registry)
        } else {
            panic!("registries len (is 0) should be >= 1");
        };
        let context = Context::new();

        let (_, context) = InjectTransient::<Request>::resolve(registry.clone(), context).unwrap();
        InjectTransient::<Request>::resolve(registry, context).unwrap();

        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 2);
    }
}
