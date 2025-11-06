use froodi::{
    autowired::__GLOBAL_ENTRY_GETTERS,
    registry, Config, Container,
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
    fn inst(Inject(b): Inject<B>, Inject(c): Inject<C>) -> Result<Self, InstantiateErrorKind> {
        Ok(Self(b, c))
    }

    fn fin(_val: Arc<Self>) {}
}

#[test]
fn test_global_entries_count() {
    assert_eq!(__GLOBAL_ENTRY_GETTERS.len(), 3);
}

#[test]
fn test_global_entries_resolve() {
    let container = Container::new_with_start_scope(registry! {}, Request);

    container.get::<C>().unwrap();
    container.get::<B>().unwrap();
    container.get::<A>().unwrap();
    container.get::<D>().unwrap_err();

    container.get_transient::<C>().unwrap();
    container.get_transient::<B>().unwrap();
    container.get_transient::<A>().unwrap();
    container.get_transient::<D>().unwrap_err();

    container.close();
}
