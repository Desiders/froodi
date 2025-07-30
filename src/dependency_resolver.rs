use alloc::sync::Arc;

use super::errors::ResolveErrorKind;
use crate::Container;

pub(crate) trait DependencyResolver: Sized {
    type Error: Into<ResolveErrorKind>;

    fn resolve(container: Container) -> Result<Self, Self::Error>;
}

pub struct Inject<Dep>(pub Arc<Dep>);

impl<Dep: Send + Sync + 'static> DependencyResolver for Inject<Dep> {
    type Error = ResolveErrorKind;

    fn resolve(container: Container) -> Result<Self, Self::Error> {
        container.get().map(Inject)
    }
}

pub struct InjectTransient<Dep>(pub Dep);

impl<Dep: 'static> DependencyResolver for InjectTransient<Dep> {
    type Error = ResolveErrorKind;

    fn resolve(container: Container) -> Result<Self, Self::Error> {
        container.get_transient().map(InjectTransient)
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
            fn resolve(container: Container) -> Result<Self, Self::Error> {
                Ok(($($ty::resolve(container.clone()).map_err(Into::into)?,)*))
            }
        }
    };
}

all_the_tuples!(impl_dependency_resolver);

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{DependencyResolver, Inject, InjectTransient};
    use crate::{errors::InstantiateErrorKind, instance, scope::DefaultScope::*, Container, RegistriesBuilder};

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

        let container = Container::new(registries_builder);

        let request_1 = Inject::<Request>::resolve(container.clone()).unwrap();
        let request_2 = Inject::<Request>::resolve(container.clone()).unwrap();
        let _ = Inject::<Instance>::resolve(container).unwrap();

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

        let container = Container::new(registries_builder);

        let _ = InjectTransient::<Request>::resolve(container.clone()).unwrap();
        InjectTransient::<Request>::resolve(container).unwrap();

        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 2);
    }
}
