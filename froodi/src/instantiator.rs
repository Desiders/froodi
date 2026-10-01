use alloc::{boxed::Box, collections::btree_set::BTreeSet};
use core::any::Any;
use tracing::debug;

use super::{
    dependency_resolver::DependencyResolver,
    errors::{InstantiateErrorKind, InstantiatorErrorKind},
    service::{service_fn, BoxCloneService, Service},
};
#[cfg(feature = "compiled")]
mod compiled;

#[cfg(feature = "compiled")]
use crate::compiled::RegistrationId;
use crate::{
    dependency::Dependency,
    utils::thread_safety::{SendSafety, SyncSafety},
    Container, ResolveErrorKind,
};
#[cfg(feature = "compiled")]
use compiled::ErasedInstantiator;

pub trait Instantiator<Deps>: Clone + 'static
where
    Deps: DependencyResolver,
{
    type Provides: 'static;
    type Error: Into<InstantiateErrorKind>;

    fn instantiate(&mut self, dependencies: Deps) -> Result<Self::Provides, Self::Error>;

    #[must_use]
    fn dependencies() -> BTreeSet<Dependency>;

    #[cfg(feature = "compiled")]
    #[doc(hidden)]
    fn instantiate_compiled(
        &self,
        container: &Container,
        edges: &[RegistrationId],
    ) -> Result<Self::Provides, InstantiatorErrorKind<ResolveErrorKind, InstantiateErrorKind>> {
        let dependencies = Deps::resolve_compiled(container, &mut edges.iter()).map_err(|err| InstantiatorErrorKind::Deps(err.into()))?;
        self.clone()
            .instantiate(dependencies)
            .map_err(|err| InstantiatorErrorKind::Factory(err.into()))
    }
}

pub(crate) type BoxedCloneInstantiator<DepsErr, FactoryErr> =
    BoxCloneService<Container, Box<dyn Any>, InstantiatorErrorKind<DepsErr, FactoryErr>>;

#[must_use]
pub(crate) fn boxed_instantiator<Inst, Deps>(instantiator: Inst) -> BoxedCloneInstantiator<Deps::Error, Inst::Error>
where
    Inst: Instantiator<Deps> + SendSafety + SyncSafety,
    Deps: DependencyResolver,
{
    BoxCloneService(Box::new(service_fn({
        move |container| {
            let dependencies = match Deps::resolve(&container) {
                Ok(dependencies) => dependencies,
                Err(err) => return Err(InstantiatorErrorKind::Deps(err)),
            };
            let dependency = match instantiator.clone().instantiate(dependencies) {
                Ok(dependency) => dependency,
                Err(err) => return Err(InstantiatorErrorKind::Factory(err)),
            };

            debug!("Resolved");

            Ok(Box::new(dependency) as _)
        }
    })))
}

#[must_use]
pub(crate) fn boxed_container_instantiator() -> BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind> {
    BoxCloneService(Box::new(service_fn(move |container| Ok(Box::new(container) as _))))
}

macro_rules! impl_instantiator {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(non_snake_case)]
        impl<F, Response, Err, $($ty,)*> Instantiator<($($ty,)*)> for F
        where
            F: FnMut($($ty,)*) -> Result<Response, Err> + Clone + 'static,
            Response: 'static,
            Err: Into<InstantiateErrorKind>,
            $( $ty: DependencyResolver + SendSafety + 'static, )*
        {
            type Provides = Response;
            type Error = Err;

            #[inline]
            fn instantiate(&mut self, ($($ty,)*): ($($ty,)*)) -> Result<Self::Provides, Self::Error> {
                self($($ty,)*)
            }

            #[inline]
            fn dependencies() -> BTreeSet<Dependency> {
                BTreeSet::from_iter([
                    $(
                        Dependency {
                            type_info: $ty::type_info(),
                        }
                    ),*
                ])
            }
        }
    };
}

all_the_tuples!(impl_instantiator);

/// Wrapper to create an instantiator that just returns passed value.
/// It can be used when the value was created outside the container.
#[inline]
#[must_use]
pub const fn instance<T: Clone + 'static>(val: T) -> impl Instantiator<(), Provides = T, Error = InstantiateErrorKind> {
    move || Ok(val.clone())
}

/// Creates a `Box<dyn Trait>` from a value, optionally including supertraits.
///
/// # Syntax
/// ```text
/// boxed!(value; Trait [+ SuperTrait1 [+ SuperTrait2 ...]])
/// ```
///
/// # Examples
/// ```rust
/// use froodi::boxed;
///
/// trait UserRepo {}
///
/// struct PostgresUserRepo;
///
/// impl UserRepo for PostgresUserRepo {}
///
/// // Single trait
/// let repo1: Box<dyn UserRepo> = boxed!(PostgresUserRepo; UserRepo);
///
/// // Trait with supertraits
/// let repo2: Box<dyn UserRepo + Send + Sync> = boxed!(PostgresUserRepo; UserRepo + Send + Sync);
/// ```
#[macro_export]
macro_rules! boxed {
    ($val:expr ; $tr:tt $($super_traits:tt)*) => {{
        use $crate::macros_utils::aliases::Box;
        Box::new($val) as Box<dyn $tr $($super_traits)*>
    }};
}

#[derive(Clone)]
pub(crate) enum RegistrationInstantiator {
    Dynamic(BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>),
    #[cfg(feature = "compiled")]
    Compiled(ErasedInstantiator),
}

impl From<BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>> for RegistrationInstantiator {
    fn from(inst: BoxedCloneInstantiator<ResolveErrorKind, InstantiateErrorKind>) -> Self {
        Self::Dynamic(inst)
    }
}

impl RegistrationInstantiator {
    pub(crate) fn call(&self, container: Container) -> Result<Box<dyn Any>, InstantiatorErrorKind<ResolveErrorKind, InstantiateErrorKind>> {
        match self {
            Self::Dynamic(inst) => Service::call(&mut inst.clone(), container),
            #[cfg(feature = "compiled")]
            Self::Compiled(inst) => inst.call(container),
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{boxed_instantiator, DependencyResolver, InstantiateErrorKind, Instantiator};
    use crate::{
        inject::{Inject, InjectTransient},
        registry,
        scope::DefaultScope::*,
        service::Service as _,
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

    struct Request(bool);
    struct Response(bool);

    #[test]
    #[allow(dead_code)]
    fn test_factory_helper() {
        fn resolver<Deps: DependencyResolver, F: Instantiator<Deps>>(_f: F) {}

        fn resolver_with_dep<Deps: DependencyResolver>() {
            resolver(|| Ok::<_, InstantiateErrorKind>(()));
        }
    }

    #[test]
    #[traced_test]
    fn test_boxed_instantiator_factory() {
        let instantiator_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let instantiator_response_call_count = RcThreadSafety::new(AtomicU8::new(0));

        let mut instantiator_response = boxed_instantiator({
            let instantiator_response_call_count = instantiator_response_call_count.clone();
            move |InjectTransient(Request(val_1)), InjectTransient(Request(val_2))| {
                assert_eq!(val_1, val_2);

                instantiator_response_call_count.fetch_add(1, Ordering::SeqCst);

                debug!("Call instantiator response");
                Ok::<_, InstantiateErrorKind>(Response(val_1))
            }
        });

        let container = Container::new(registry! {
            scope(App) [
                provide({
                    let instantiator_request_call_count = instantiator_request_call_count.clone();
                    move || {
                        instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                        debug!("Call instantiator request");
                        Ok::<_, InstantiateErrorKind>(Request(true))
                    }
                }),
            ]
        });

        let response_1 = instantiator_response.call(container.clone()).unwrap();
        let response_2 = instantiator_response.call(container).unwrap();

        assert!(response_1.downcast::<Response>().unwrap().0);
        assert!(response_2.downcast::<Response>().unwrap().0);
        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 4);
        assert_eq!(instantiator_response_call_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    #[traced_test]
    fn test_boxed_instantiator_cached_factory() {
        let instantiator_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let instantiator_response_call_count = RcThreadSafety::new(AtomicU8::new(0));

        let mut instantiator_response = boxed_instantiator({
            let instantiator_response_call_count = instantiator_response_call_count.clone();
            move |val_1: Inject<Request>, val_2: Inject<Request>| {
                assert_eq!(val_1.0 .0, val_2.0 .0);

                instantiator_response_call_count.fetch_add(1, Ordering::SeqCst);

                debug!("Call instantiator response");
                Ok::<_, InstantiateErrorKind>(Response(val_1.0 .0))
            }
        });

        let container = Container::new(registry! {
            scope(App) [
                provide({
                    let instantiator_request_call_count = instantiator_request_call_count.clone();
                    move || {
                        instantiator_request_call_count.fetch_add(1, Ordering::SeqCst);

                        debug!("Call instantiator request");
                        Ok::<_, InstantiateErrorKind>(Request(true))
                    }
                }),
            ]
        });

        let response_1 = instantiator_response.call(container.clone()).unwrap();
        let response_2 = instantiator_response.call(container.clone()).unwrap();
        let response_3 = instantiator_response.call(container).unwrap();

        assert!(response_1.downcast::<Response>().unwrap().0);
        assert!(response_2.downcast::<Response>().unwrap().0);
        assert!(response_3.downcast::<Response>().unwrap().0);
        assert_eq!(instantiator_request_call_count.load(Ordering::SeqCst), 1);
        // We don't cache instantiator provides of main factory here, we do it in container
        assert_eq!(instantiator_response_call_count.load(Ordering::SeqCst), 3);
    }
}
