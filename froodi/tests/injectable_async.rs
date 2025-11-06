use froodi::{async_impl::autowired::__GLOBAL_ASYNC_ENTRY_GETTERS, Config, DefaultScope::App, Inject, InstantiateErrorKind};
use froodi_macros::injectable;
use std::sync::Arc;

#[derive(Clone)]
struct C;

#[injectable]
impl C {
    #[provide(App)]
    async fn inst() -> Result<Self, InstantiateErrorKind> {
        Ok(Self)
    }
}

#[derive(Clone)]
struct B;

#[injectable]
impl B {
    #[provide(App, finalizer = B::fin)]
    async fn inst() -> Result<Self, InstantiateErrorKind> {
        Ok(Self)
    }

    async fn fin(_val: Arc<Self>) {}
}

#[derive(Clone)]
#[allow(dead_code)]
struct A(Arc<B>, Arc<C>);

#[injectable]
impl A {
    #[provide(App, finalizer = A::fin, config = Config::default())]
    async fn inst(Inject(b): Inject<B>, Inject(c): Inject<C>) -> Result<Self, InstantiateErrorKind> {
        Ok(Self(b, c))
    }

    async fn fin(_val: Arc<Self>) {}
}

#[test]
fn test_global_entries_count() {
    assert_eq!(__GLOBAL_ASYNC_ENTRY_GETTERS.len(), 3);
}
