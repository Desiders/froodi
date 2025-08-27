use alloc::{boxed::Box, collections::btree_set::BTreeSet, sync::Arc, vec};
use core::future::Future;
use dptree::{
    di::{Asyncify, CompiledFn, DependencyMap, Injectable as InjectableTrait},
    Type,
};

#[cfg(feature = "async")]
use crate::async_impl::Container as AsyncContainer;
use crate::{
    dependency_resolver::DependencyResolver,
    Container,
    DefaultScope::{self, Request as RequestScope},
    ResolveErrorKind, Scope,
};

#[derive(Clone)]
pub struct MapInject<T>(pub Arc<T>);

impl<T: Send + Sync + 'static> DependencyResolver for MapInject<T> {
    type Error = ResolveErrorKind;

    fn resolve(container: &Container) -> Result<Self, Self::Error> {
        let map = container.get::<DependencyMap>()?;
        match map.try_get::<T>() {
            Some(val) => Ok(Self(val)),
            None => Err(Self::Error::NoInstantiator),
        }
    }

    #[cfg(feature = "async")]
    async fn resolve_async(container: &AsyncContainer) -> Result<Self, Self::Error> {
        let map = container.get::<DependencyMap>().await?;
        match map.try_get::<T>() {
            Some(val) => Ok(Self(val)),
            None => Err(Self::Error::NoInstantiator),
        }
    }
}

pub struct Injectable<F, const PREFER_SYNC_OVER_ASYNC: bool = true>(F);

impl<F> Injectable<F, true> {
    #[inline]
    pub const fn new(this: F) -> Self {
        Self(this)
    }

    #[inline]
    pub const fn new_sync_over_async(this: F) -> Self {
        Self::new(this)
    }
}

impl<F> Injectable<F, false> {
    #[inline]
    pub const fn new_async_over_sync(this: F) -> Self {
        Self(this)
    }
}

macro_rules! impl_setup {
    (
        $StructName:ident,
        $ContainerType:ty
    ) => {
        pub struct $StructName<WithScope> {
            container: $ContainerType,
            scope: WithScope,
        }

        impl<WithScope> InjectableTrait<Option<$ContainerType>, ()> for $StructName<WithScope>
        where
            WithScope: Scope + Clone + Send + Sync + 'static,
        {
            /// # Panics
            /// Function enters child specified scope and panics on failure
            fn inject<'a>(&'a self, map: &'a DependencyMap) -> CompiledFn<'a, Option<$ContainerType>> {
                Arc::new(move || {
                    Box::pin(async move {
                        let mut context = crate::Context::new();
                        context.insert(map.clone());

                        self.container
                            .clone()
                            .enter()
                            .with_scope(self.scope.clone())
                            .with_context(context)
                            .build()
                            .ok()
                    })
                })
            }

            fn input_types() -> BTreeSet<Type> {
                BTreeSet::from_iter(vec![Type::of::<()>()])
            }
        }
    };
}
impl_setup!(Setup, Container);

#[cfg(feature = "async")]
impl_setup!(AsyncSetup, AsyncContainer);

#[inline]
#[must_use]
pub const fn setup<WithScope>(container: Container, scope: WithScope) -> Setup<WithScope>
where
    WithScope: Scope + Clone + Send + Sync + 'static,
{
    Setup { container, scope }
}

#[inline]
#[must_use]
pub const fn setup_default(container: Container) -> Setup<DefaultScope> {
    setup(container, RequestScope)
}

#[inline]
#[must_use]
#[cfg(feature = "async")]
pub const fn setup_async<WithScope>(container: AsyncContainer, scope: WithScope) -> AsyncSetup<WithScope>
where
    WithScope: Scope + Clone + Send + Sync + 'static,
{
    AsyncSetup { container, scope }
}

#[inline]
#[must_use]
#[cfg(feature = "async")]
pub const fn setup_async_default(container: AsyncContainer) -> AsyncSetup<DefaultScope> {
    setup_async(container, RequestScope)
}

macro_rules! impl_injectable {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(non_snake_case)]
        impl<F, Output, Fut, const PREFER_SYNC_OVER_ASYNC: bool, $($ty,)*> InjectableTrait<Output, ($($ty,)*)> for Injectable<F, PREFER_SYNC_OVER_ASYNC>
        where
            F: Fn($($ty,)*) -> Fut + Sync,
            Fut: Future<Output = Output> + Send + 'static,
            Output: Send + 'static,
            $($ty: DependencyResolver + Send + 'static),*
        {
            #[cfg(not(feature = "async"))]
            #[allow(non_snake_case, unused_variables)]
            fn inject<'a>(&'a self, map: &'a DependencyMap) -> CompiledFn<'a, Output> {
                let Self(this) = self;
                Arc::new( move ||  {
                    let container = core::borrow::Borrow::<Container>::borrow(&map.get()).clone();
                    $( let $ty = $ty::resolve(&container).map_err(Into::into).unwrap(); )*
                    Box::pin(async move {
                        let res = this( $( $ty ),* ).await;
                        container.close();
                        res
                    })
                })
            }

            #[cfg(feature = "async")]
            #[allow(non_snake_case, unused_variables)]
            fn inject<'a>(&'a self, map: &'a DependencyMap) -> CompiledFn<'a, Output> {
                let Self(this) = self;
                Arc::new(move || Box::pin(async move {
                    let container_sync = map.try_get::<Container>();
                    let container_async = map.try_get::<AsyncContainer>();

                    $( let $ty =
                        if PREFER_SYNC_OVER_ASYNC {
                            match container_sync {
                                Some(ref container) => $ty::resolve(&container).map_err(Into::into).unwrap(),
                                None => match container_async {
                                    Some(ref container) => $ty::resolve_async(&container).await.map_err(Into::into).unwrap(),
                                    None => panic!("sync and async containers are not found"),
                                },
                            }
                        } else {
                            match container_async {
                                Some(ref container) => $ty::resolve_async(&container).await.map_err(Into::into).unwrap(),
                                None => match container_sync {
                                    Some(ref container) => $ty::resolve(&container).map_err(Into::into).unwrap(),
                                    None => panic!("sync and async containers are not found"),
                                },
                            }
                        };
                    )*
                    let res = this( $( $ty ),* ).await;
                    if let Some(container) = container_async {
                        container.close().await;
                    }
                    if let Some(container) = container_sync {
                        container.close();
                    }
                    res
                }))
            }

            fn input_types() -> BTreeSet<Type> {
                BTreeSet::from_iter(vec![
                    $(Type::of::<$ty>()),*
                ])
            }
        }

        #[allow(non_snake_case)]
        impl<F, Output, const PREFER_SYNC_OVER_ASYNC: bool, $($ty,)*> InjectableTrait<Output, ($($ty,)*)> for Injectable<Asyncify<F>, PREFER_SYNC_OVER_ASYNC>
        where
            F: Fn($($ty,)*) -> Output + Sync,
            Output: Send + 'static,
            $($ty: DependencyResolver + Send + 'static),*
        {
            #[cfg(not(feature = "async"))]
            #[allow(non_snake_case, unused_variables)]
            fn inject<'a>(&'a self, map: &'a DependencyMap) -> CompiledFn<'a, Output> {
                let Self(Asyncify(this)) = self;
                Arc::new( move ||  {
                    let container = core::borrow::Borrow::<Container>::borrow(&map.get()).clone();
                    $( let $ty = $ty::resolve(&container).map_err(Into::into).unwrap(); )*
                    Box::pin(async move {
                        let res = this( $( $ty ),* );
                        container.close();
                        res
                    })
                })
            }

            #[cfg(feature = "async")]
            #[allow(non_snake_case, unused_variables)]
            fn inject<'a>(&'a self, map: &'a DependencyMap) -> CompiledFn<'a, Output> {
                let Self(Asyncify(this)) = self;
                Arc::new(move || Box::pin(async move {
                    let container_sync = map.try_get::<Container>();
                    let container_async = map.try_get::<AsyncContainer>();

                    $( let $ty =
                        if PREFER_SYNC_OVER_ASYNC {
                            match container_sync {
                                Some(ref container) => $ty::resolve(&container).map_err(Into::into).unwrap(),
                                None => match container_async {
                                    Some(ref container) => $ty::resolve_async(&container).await.map_err(Into::into).unwrap(),
                                    None => panic!("sync and async containers are not found"),
                                },
                            }
                        } else {
                            match container_async {
                                Some(ref container) => $ty::resolve_async(&container).await.map_err(Into::into).unwrap(),
                                None => match container_sync {
                                    Some(ref container) => $ty::resolve(&container).map_err(Into::into).unwrap(),
                                    None => panic!("sync and async containers are not found"),
                                },
                            }
                        };
                    )*
                    let res = this( $( $ty ),* );
                    if let Some(container) = container_async {
                        container.close().await;
                    }
                    if let Some(container) = container_sync {
                        container.close();
                    }
                    res
                }))
            }

            fn input_types() -> BTreeSet<Type> {
                BTreeSet::from_iter(vec![
                    $(Type::of::<$ty>()),*
                ])
            }
        }
    };
}

all_the_tuples!(impl_injectable);

#[cfg(test)]
mod tests {
    use super::{Asyncify, Injectable, InjectableTrait, MapInject};
    use crate::{Inject, InjectTransient};

    #[test]
    fn test_bounds() {
        fn impl_bounds<Output: 'static, Args, T: InjectableTrait<Output, Args>>(_val: T) {}

        impl_bounds(Injectable::new(
            async |_val1: MapInject<()>, _val2: Inject<()>, _val3: InjectTransient<()>| (),
        ));
        impl_bounds(Injectable::new(Asyncify(
            |_val1: MapInject<()>, _val2: Inject<()>, _val3: InjectTransient<()>| (),
        )));
    }
}
