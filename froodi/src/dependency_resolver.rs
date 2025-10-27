#[cfg(feature = "async")]
use core::future::Future;

use super::errors::ResolveErrorKind;
#[cfg(feature = "async")]
use crate::async_impl::Container as AsyncContainer;
use crate::{any::TypeInfo, utils::thread_safety::SendSafety, Container};

pub trait DependencyResolver: Sized {
    type Error: Into<ResolveErrorKind>;

    fn resolve(container: &Container) -> Result<Self, Self::Error>;

    #[cfg(feature = "async")]
    fn resolve_async(container: &AsyncContainer) -> impl Future<Output = Result<Self, Self::Error>> + SendSafety;

    #[inline]
    #[must_use]
    fn type_info() -> TypeInfo
    where
        Self: 'static,
    {
        TypeInfo::of::<Self>()
    }
}

macro_rules! impl_dependency_resolver {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(non_snake_case, unused_mut)]
        impl<$($ty,)*> DependencyResolver for ($($ty,)*)
        where
            $( $ty: DependencyResolver + SendSafety, )*
        {
            type Error = ResolveErrorKind;

            #[inline]
            #[allow(unused_variables)]
            fn resolve(container: &Container) -> Result<Self, Self::Error> {
                Ok(($($ty::resolve(container).map_err(Into::into)?,)*))
            }

            #[inline]
            #[allow(unused_variables)]
            #[cfg(feature = "async")]
            async fn resolve_async(container: &AsyncContainer) -> Result<Self, Self::Error> {
                Ok(($($ty::resolve_async(container).await.map_err(Into::into)?,)*))
            }
        }
    };
}

all_the_tuples!(impl_dependency_resolver);

#[cfg(test)]
mod tests {
    extern crate std;

    use super::DependencyResolver;
    use crate::{
        errors::InstantiateErrorKind,
        inject::{Inject, InjectTransient},
        instance, registry,
        scope::DefaultScope::*,
        utils::thread_safety::RcThreadSafety,
        Container,
    };

    use alloc::{
        format,
        string::{String, ToString as _},
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
        let instantiator_request_call_count = RcThreadSafety::new(AtomicU8::new(0));

        let container = Container::new(registry! {
            scope(App) [
                provide({
                    let instantiator_request_call_count = instantiator_request_call_count.clone();
                    move || {
                        instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                        debug!("Call instantiator request");
                        Ok::<_, InstantiateErrorKind>(Request)
                    }
                }),
                provide(instance(Instance)),
            ]
        });

        let request_1 = Inject::<Request>::resolve(&container).unwrap();
        let request_2 = Inject::<Request>::resolve(&container).unwrap();
        let _ = Inject::<Instance>::resolve(&container).unwrap();

        assert!(RcThreadSafety::ptr_eq(&request_1.0, &request_2.0));
        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    #[traced_test]
    fn test_transient_resolve() {
        let instantiator_request_call_count = RcThreadSafety::new(AtomicU8::new(0));

        let container = Container::new(registry! {
            scope(App) [
                provide({
                    let instantiator_request_call_count = instantiator_request_call_count.clone();
                    move || {
                        instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                        debug!("Call instantiator request");
                        Ok::<_, InstantiateErrorKind>(Request)
                    }
                }),
            ]
        });

        let _ = InjectTransient::<Request>::resolve(&container).unwrap();
        InjectTransient::<Request>::resolve(&container).unwrap();

        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 2);
    }
}
