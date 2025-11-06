use froodi::{
    async_impl::{autowired::__GLOBAL_ASYNC_ENTRY_GETTERS, Container},
    async_registry,
    autowired::__GLOBAL_ENTRY_GETTERS,
    Config,
    DefaultScope::{App, Request, Session},
    Inject, InstantiateErrorKind,
};
use froodi_macros::injectable;
use std::sync::Arc;

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

    fn fin(_val: Arc<Self>) {}
}

#[derive(Clone)]
#[allow(dead_code)]
struct A(Arc<B>, Arc<C>);

#[injectable]
impl A {
    #[provide(Request, finalizer = A::fin, config = Config::default())]
    async fn inst(Inject(b): Inject<B>, Inject(c): Inject<C>) -> Result<Self, InstantiateErrorKind> {
        Ok(Self(b, c))
    }

    async fn fin(_val: Arc<Self>) {}
}

#[test]
fn test_global_entries_count() {
    assert_eq!(__GLOBAL_ENTRY_GETTERS.len(), 2);
}

#[test]
fn test_global_async_entries_count() {
    assert_eq!(__GLOBAL_ASYNC_ENTRY_GETTERS.len(), 1);
}

#[tokio::test]
async fn test_global_entries_resolve() {
    let container = Container::new_with_start_scope(async_registry! {}, Request);

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
