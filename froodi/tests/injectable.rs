use froodi::{autowired::__GLOBAL_ENTRY_GETTERS, Config, DefaultScope::App, Inject, InstantiateErrorKind};
use froodi_macros::injectable;
use std::sync::Arc;

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
    #[provide(App, finalizer = B::fin)]
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
    #[provide(App, finalizer = A::fin, config = Config::default())]
    fn inst(Inject(b): Inject<B>, Inject(c): Inject<C>) -> Result<Self, InstantiateErrorKind> {
        Ok(Self(b, c))
    }

    fn fin(_val: Arc<Self>) {}
}

#[test]
fn test_global_entries_count() {
    assert_eq!(__GLOBAL_ENTRY_GETTERS.len(), 3);
}
