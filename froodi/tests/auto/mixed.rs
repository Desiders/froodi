#![no_std]

extern crate alloc;

use froodi::{
    async_impl::Container,
    async_registry,
    utils::thread_safety::RcThreadSafety,
    Config,
    DefaultScope::{App, Request, Session},
    Inject, InstantiateErrorKind,
};
use froodi_auto::{
    entry_getters::{__ASYNC_ENTRY_GETTERS, __ENTRY_GETTERS},
    injectable, AutoRegistriesWithSync as _,
};

#[derive(Debug, Clone)]
struct D;

#[derive(Clone)]
struct C;

#[injectable]
impl C {
    #[provide(App)]
    fn inst() -> Result<Self, InstantiateErrorKind> {
        Ok(Self)
    }
}

#[derive(Clone)]
struct B;

#[injectable]
impl B {
    #[provide(Session, finalizer = B::fin)]
    fn inst() -> Result<Self, InstantiateErrorKind> {
        Ok(Self)
    }

    fn fin(_val: RcThreadSafety<Self>) {}
}

#[derive(Clone)]
#[allow(dead_code)]
struct A(RcThreadSafety<B>, RcThreadSafety<C>);

#[injectable]
impl A {
    #[provide(Request, finalizer = A::fin, config = Config::default())]
    async fn inst(Inject(b): Inject<B>, Inject(c): Inject<C>) -> Result<Self, InstantiateErrorKind> {
        Ok(Self(b, c))
    }

    async fn fin(_val: RcThreadSafety<Self>) {}
}

#[test]
fn test_entries_count() {
    assert_eq!(__ENTRY_GETTERS.len(), 2);
    assert_eq!(__ASYNC_ENTRY_GETTERS.len(), 1);
}

#[tokio::test]
async fn test_entries() {
    let container = Container::new_with_start_scope(async_registry! {}.provide_auto_registries_with_sync(), Request);

    container.get::<C>().await.unwrap();
    container.get::<B>().await.unwrap();
    container.get::<A>().await.unwrap();
    container.get::<D>().await.unwrap_err();

    container.get_transient::<C>().await.unwrap();
    container.get_transient::<B>().await.unwrap();
    container.get_transient::<A>().await.unwrap();
    container.get_transient::<D>().await.unwrap_err();

    container.close().await;
}
