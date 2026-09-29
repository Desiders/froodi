use froodi_compile::{registry, Container, DefaultScope::App, Inject, InjectTransient, InstantiateErrorKind};

struct A;
struct B;

type Alias = A;

fn main() {
    let fragment = registry! { provide(App, |_: Inject<B>| Ok::<_, InstantiateErrorKind>(A)) };
    let _ = Container::try_new(registry! {
        provide(App, |_: InjectTransient<Alias>| Ok::<_, InstantiateErrorKind>(B)),
        extend(fragment),
    });
}

#[test]
fn instantiates() {
    main();
}
