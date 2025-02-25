use alloc::{boxed::Box, sync::Arc};
use core::{any::type_name, future::Future};
use futures_util::future::BoxFuture;
use tracing::{debug_span, error, warn, Instrument as _};

use super::{context::Context, instantiator::Request, registry::Registry, service::Service as _};
use crate::errors::{InstantiatorErrorKind, ResolveErrorKind};

pub(crate) trait DependencyResolver: Send + Sized {
    type Error: Into<ResolveErrorKind>;
    type Future: Future<Output = Result<Self, Self::Error>> + Send;

    #[cfg(feature = "async_tokio")]
    fn resolve(registry: Arc<Registry>, context: Arc<tokio::sync::Mutex<Context>>) -> Self::Future;
}

pub(crate) struct Inject<Dep>(pub(crate) Dep);

impl<Dep: Send + 'static> DependencyResolver for Inject<Dep> {
    type Error = ResolveErrorKind;
    type Future = BoxFuture<'static, Result<Self, Self::Error>>;

    #[cfg(feature = "async_tokio")]
    fn resolve(registry: Arc<Registry>, context: Arc<tokio::sync::Mutex<Context>>) -> Self::Future {
        Box::pin(
            async move {
                let Some((mut instantiator, config)) = registry.get_instantiator::<Dep>() else {
                    warn!("Instantiator not found in registry");
                    return Err(ResolveErrorKind::NoFactory);
                };

                let dependency = match instantiator.call(Request::new(registry, config, context)).await {
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
            .instrument(debug_span!("resolve", dependency = type_name::<Dep>())),
        )
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
