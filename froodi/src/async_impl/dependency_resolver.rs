use alloc::sync::Arc;
use core::future::Future;

use super::Container;
use crate::errors::ResolveErrorKind;

pub trait DependencyResolver: Sized {
    type Error: Into<ResolveErrorKind>;

    fn resolve(container: Container) -> impl Future<Output = Result<Self, Self::Error>> + Send;
}

pub struct Inject<Dep>(pub Arc<Dep>);

impl<Dep: Send + Sync + 'static> DependencyResolver for Inject<Dep> {
    type Error = ResolveErrorKind;

    async fn resolve(container: Container) -> Result<Self, Self::Error> {
        container.get().await.map(Self)
    }
}

pub struct InjectTransient<Dep>(pub Dep);

impl<Dep: 'static> DependencyResolver for InjectTransient<Dep> {
    type Error = ResolveErrorKind;

    async fn resolve(container: Container) -> Result<Self, Self::Error> {
        container.get_transient().await.map(Self)
    }
}

macro_rules! impl_dependency_resolver {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(non_snake_case, unused_mut)]
        impl<$($ty,)*> DependencyResolver for ($($ty,)*)
        where
            $( $ty: DependencyResolver + Send, )*
        {
            type Error = ResolveErrorKind;

            #[inline]
            #[allow(unused_variables)]
            async fn resolve(container: Container) -> Result<Self, Self::Error> {
                Ok(($($ty::resolve(container.clone()).await.map_err(Into::into)?,)*))
            }
        }
    };
}

all_the_tuples!(impl_dependency_resolver);

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{Container, DependencyResolver, Inject, InjectTransient};
    use crate::{async_impl::RegistriesBuilder, errors::InstantiateErrorKind, instance, scope::DefaultScope::*};

    use alloc::{
        format,
        string::{String, ToString as _},
        sync::Arc,
    };
    use core::sync::atomic::{AtomicU8, Ordering};
    use tracing::debug;
    use tracing_test::traced_test;

    struct SyncRequest;
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

    #[tokio::test]
    #[traced_test]
    async fn test_scoped_resolve() {
        let instantiator_request_call_count = Arc::new(AtomicU8::new(0));
        let sync_instantiator_request_call_count = Arc::new(AtomicU8::new(0));

        let registries_builder = RegistriesBuilder::new()
            .provide(
                {
                    let instantiator_request_call_count = sync_instantiator_request_call_count.clone();
                    move || {
                        instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                        debug!("Call sync instantiator request");
                        Ok::<_, InstantiateErrorKind>(SyncRequest)
                    }
                },
                App,
            )
            .provide_async(
                {
                    let instantiator_request_call_count = instantiator_request_call_count.clone();
                    move || {
                        let instantiator_request_call_count = instantiator_request_call_count.clone();
                        async move {
                            instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                            debug!("Call instantiator request");
                            Ok::<_, InstantiateErrorKind>(Request)
                        }
                    }
                },
                App,
            )
            .provide(instance(Instance), App);

        let container = Container::new(registries_builder);

        let request_1 = Inject::<Request>::resolve(container.clone()).await.unwrap();
        let request_2 = Inject::<Request>::resolve(container.clone()).await.unwrap();
        let sync_request_1 = Inject::<SyncRequest>::resolve(container.clone()).await.unwrap();
        let sync_request_2 = Inject::<SyncRequest>::resolve(container.clone()).await.unwrap();
        let _ = Inject::<Instance>::resolve(container).await.unwrap();

        assert!(Arc::ptr_eq(&sync_request_1.0, &sync_request_2.0));
        assert!(Arc::ptr_eq(&request_1.0, &request_2.0));
        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(sync_instantiator_request_call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_transient_resolve() {
        let instantiator_request_call_count = Arc::new(AtomicU8::new(0));
        let sync_instantiator_request_call_count = Arc::new(AtomicU8::new(0));

        let registries_builder = RegistriesBuilder::new()
            .provide(
                {
                    let instantiator_request_call_count = sync_instantiator_request_call_count.clone();
                    move || {
                        instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                        debug!("Call sync instantiator request");
                        Ok::<_, InstantiateErrorKind>(SyncRequest)
                    }
                },
                App,
            )
            .provide_async(
                {
                    let instantiator_request_call_count = instantiator_request_call_count.clone();
                    move || {
                        let instantiator_request_call_count = instantiator_request_call_count.clone();
                        async move {
                            {
                                instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                                debug!("Call instantiator request");
                                Ok::<_, InstantiateErrorKind>(Request)
                            }
                        }
                    }
                },
                App,
            );

        let container = Container::new(registries_builder);

        let _ = InjectTransient::<Request>::resolve(container.clone()).await.unwrap();
        InjectTransient::<Request>::resolve(container.clone()).await.unwrap();
        let _ = InjectTransient::<SyncRequest>::resolve(container.clone()).await.unwrap();
        InjectTransient::<SyncRequest>::resolve(container).await.unwrap();

        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 2);
        assert_eq!(sync_instantiator_request_call_count.load(Ordering::SeqCst), 2);
    }
}
