use alloc::{boxed::Box, sync::Arc};
use core::{any::type_name, future::Future};
use futures_util::future::BoxFuture;
use tracing::{debug, error, instrument};

use super::{
    context::Context,
    instantiator::{InstantiateErrorKind, InstantiatorErrorKind, Request},
    registry::Registry,
    service::base::Service as _,
};

#[derive(Debug)]
pub(crate) enum ResolveErrorKind {
    NoFactory,
    Factory(InstantiatorErrorKind<Box<ResolveErrorKind>, InstantiateErrorKind>),
}

pub(crate) trait DependencyResolver: Send + Sized {
    type Error: Into<ResolveErrorKind>;
    type Future: Future<Output = Result<Self, Self::Error>> + Send;

    #[cfg(feature = "async_tokio")]
    fn resolve(registry: Arc<Registry>, context: Arc<tokio::sync::Mutex<Context>>) -> Self::Future;
}

pub(crate) struct Inject<Dep, const CACHABLE: bool = true>(pub(crate) Dep);

impl<Dep: Clone + Send + 'static> DependencyResolver for Inject<Dep, true> {
    type Error = ResolveErrorKind;
    type Future = BoxFuture<'static, Result<Self, Self::Error>>;

    #[instrument(skip_all, fields(dependency = type_name::<Dep>()))]
    #[cfg(feature = "async_tokio")]
    fn resolve(registry: Arc<Registry>, context: Arc<tokio::sync::Mutex<Context>>) -> Self::Future {
        Box::pin(async move {
            if let Some(dependency) = context.lock().await.get::<Dep>() {
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

            let dependency = match instantiator
                .call(Request::new(registry, context.clone()))
                .await
            {
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
                context.lock().await.insert(dependency.clone());

                debug!("Dependency cached");
            }

            Ok(Self(dependency))
        })
    }
}

impl<Dep: Send + 'static> DependencyResolver for Inject<Dep, false> {
    type Error = ResolveErrorKind;
    type Future = BoxFuture<'static, Result<Self, Self::Error>>;

    #[instrument(skip_all, fields(dependency = type_name::<Dep>()))]
    #[cfg(feature = "async_tokio")]
    fn resolve(registry: Arc<Registry>, context: Arc<tokio::sync::Mutex<Context>>) -> Self::Future {
        Box::pin(async move {
            if let Some(dependency) = context.lock().await.get::<Dep>() {
                debug!("Dependency found in context");
                return Ok(Self(dependency));
            } else {
                debug!("Dependency not found in context");
            }

            let Some((mut instantiator, _config)) = registry.get_instantiator::<Dep>() else {
                debug!("Instantiator not found in registry");
                return Err(ResolveErrorKind::NoFactory);
            };

            let dependency = match instantiator.call(Request::new(registry, context)).await {
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
        })
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
            type Future = BoxFuture<'static, Result<Self, Self::Error>>;

            #[inline]
            #[allow(unused_variables)]
            #[cfg(feature = "async_tokio")]
            fn resolve(registry: Arc<Registry>, context: Arc<tokio::sync::Mutex<Context>>) -> Self::Future {
                Box::pin(async move {
                    Ok(($(
                        $ty::resolve(registry.clone(), context.clone()).await.map_err(Into::into)?,
                    )*))
                })
            }
        }
    };
}

all_the_tuples!(impl_dependency_resolver);
