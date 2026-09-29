use froodi_compile::{instance, registry, Container, DefaultScope::App, Inject, InjectTransient, InstantiateErrorKind};
#[derive(Clone)]
struct Base(usize);
type Alias = Base;
struct Left(usize);
struct Right(usize);
struct End(usize);
fn left(dep: Inject<Alias>) -> Result<Left, InstantiateErrorKind> {
    Ok(Left(dep.0 .0))
}
fn right(dep: InjectTransient<Base>) -> Result<Right, InstantiateErrorKind> {
    Ok(Right(dep.0 .0))
}
fn main() {
    let captured = 3;
    // Neither fragment is linked until final composition; declaration order is deliberately reversed.
    let fragment = registry! { provide(App, left), provide(App, right) };
    let container = Container::new(registry! {
        provide(App, move |left: Inject<Left>, right: Inject<Right>| Ok::<_, InstantiateErrorKind>(End(left.0.0 + right.0.0 + captured))),
        extend(fragment),
        provide(App, instance(Base(7))),
    });
    assert_eq!(container.get::<End>().unwrap().0, 17);
}
#[test]
fn executes() {
    main();
}
