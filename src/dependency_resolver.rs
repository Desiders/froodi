use alloc::{boxed::Box, sync::Arc};
use core::any::{type_name, TypeId};
use tracing::{debug, debug_span, error, warn};

use super::{
    cache::Cache,
    errors::{InstantiatorErrorKind, ResolveErrorKind},
    instantiator::Request,
    service::Service as _,
};
use crate::{
    cache::Resolved,
    registry::{InstantiatorInnerData, Registry},
};

pub(crate) trait DependencyResolver: Sized {
    type Error: Into<ResolveErrorKind>;

    fn resolve(registry: Arc<Registry>, cache: Cache) -> Result<(Self, Cache), Self::Error>;
}

pub struct Inject<Dep>(pub Arc<Dep>);

impl<Dep: Send + Sync + 'static> DependencyResolver for Inject<Dep> {
    type Error = ResolveErrorKind;

    fn resolve(registry: Arc<Registry>, cache: Cache) -> Result<(Self, Cache), Self::Error> {
        let span = debug_span!("resolve", dependency = type_name::<Dep>());
        let _guard = span.enter();

        let type_id = TypeId::of::<Dep>();

        if let Some(dependency) = cache.get(&type_id) {
            debug!("Found in cache");
            return Ok((Self(dependency), cache));
        }
        debug!("Not found in cache");

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

        match instantiator.call(Request::new(registry, cache)) {
            Ok((dependency, mut cache)) => match dependency.downcast::<Dep>() {
                Ok(dependency) => {
                    let dependency = Arc::new(*dependency);
                    if config.cache_provides {
                        cache.insert_rc(dependency.clone());
                        debug!("Cached");
                    }
                    if finalizer.is_some() {
                        cache.push_resolved(Resolved {
                            type_id,
                            dependency: dependency.clone(),
                        });
                        debug!("Pushed to resolved set");
                    }
                    Ok((Self(dependency), cache))
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

pub struct InjectTransient<Dep>(pub Dep);

impl<Dep: 'static> DependencyResolver for InjectTransient<Dep> {
    type Error = ResolveErrorKind;

    fn resolve(registry: Arc<Registry>, cache: Cache) -> Result<(Self, Cache), Self::Error> {
        let span = debug_span!("resolve", dependency = type_name::<Dep>());
        let _guard = span.enter();

        let type_id = TypeId::of::<Dep>();

        let Some(mut instantiator) = registry.get_instantiator(&type_id) else {
            let err = ResolveErrorKind::NoInstantiator;
            warn!("{}", err);
            return Err(err);
        };

        match instantiator.call(Request::new(registry, cache)) {
            Ok((dependency, cache)) => match dependency.downcast::<Dep>() {
                Ok(dependency) => Ok((Self(*dependency as _), cache)),
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
            fn resolve(registry: Arc<Registry>, cache: Cache) -> Result<(Self, Cache), Self::Error> {
                let mut cache = cache;
                Ok((
                    (
                        $(
                            {
                                let ($ty, updated_cache) = $ty::resolve(registry.clone(), cache).map_err(Into::into)?;
                                cache = updated_cache;
                                $ty
                            }
                        ,)*
                    ),
                    cache,
                ))
            }
        }
    };
}

all_the_tuples!(impl_dependency_resolver);

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{Cache, DependencyResolver, Inject, InjectTransient};
    use crate::{errors::InstantiateErrorKind, instance, scope::DefaultScope::*, RegistriesBuilder};

    use alloc::{
        format,
        string::{String, ToString as _},
        sync::Arc,
    };
    use core::sync::atomic::{AtomicU8, Ordering};
    use tracing::debug;
    use tracing_test::traced_test;

    struct Request;

    #[derive(Clone)]
    struct Instance;

    #[test]
    #[allow(dead_code)]
    fn test_dependency_resolver_impls() {
        fn resolver<T: DependencyResolver>() {}
        fn resolver_with_dep<Dep: Send + Sync + 'static>() {
            resolver::<Inject<Dep>>();
            resolver::<InjectTransient<Dep>>();
            resolver::<(Inject<Dep>, InjectTransient<Dep>)>();
        }
    }

    #[test]
    #[traced_test]
    fn test_scoped_resolve() {
        let instantiator_request_call_count = Arc::new(AtomicU8::new(0));

        let registries_builder = RegistriesBuilder::new()
            .provide(
                {
                    let instantiator_request_call_count = instantiator_request_call_count.clone();
                    move || {
                        instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                        debug!("Call instantiator request");
                        Ok::<_, InstantiateErrorKind>(Request)
                    }
                },
                App,
            )
            .provide(instance(Instance), App);

        let mut registries = registries_builder.build().into_iter();
        let registry = if let Some(root_registry) = registries.next() {
            Arc::new(root_registry)
        } else {
            panic!("registries len (is 0) should be >= 1");
        };

        let cache = Cache::new();

        let (request_1, cache) = Inject::<Request>::resolve(registry.clone(), cache).unwrap();
        let (request_2, cache) = Inject::<Request>::resolve(registry.clone(), cache).unwrap();
        let (_, _) = Inject::<Instance>::resolve(registry, cache).unwrap();

        assert!(Arc::ptr_eq(&request_1.0, &request_2.0));
        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    #[traced_test]
    fn test_transient_resolve() {
        let instantiator_request_call_count = Arc::new(AtomicU8::new(0));

        let registries_builder = RegistriesBuilder::new().provide(
            {
                let instantiator_request_call_count = instantiator_request_call_count.clone();
                move || {
                    instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Call instantiator request");
                    Ok::<_, InstantiateErrorKind>(Request)
                }
            },
            App,
        );

        let mut registries = registries_builder.build().into_iter();
        let registry = if let Some(root_registry) = registries.next() {
            Arc::new(root_registry)
        } else {
            panic!("registries len (is 0) should be >= 1");
        };
        let cache = Cache::new();

        let (_, cache) = InjectTransient::<Request>::resolve(registry.clone(), cache).unwrap();
        InjectTransient::<Request>::resolve(registry, cache).unwrap();

        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 2);
    }
}
