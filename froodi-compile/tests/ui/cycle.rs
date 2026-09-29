use froodi_compile::{registry, Container, DefaultScope::*, Inject, InstantiateErrorKind};

struct A;
struct B;

fn make_a(Inject(_b): Inject<B>) -> Result<A, InstantiateErrorKind> {
    Ok(A)
}

fn make_b(Inject(_a): Inject<A>) -> Result<B, InstantiateErrorKind> {
    Ok(B)
}

fn main() {
    let _container = Container::new(registry! {
        scope(App) [
            provide(make_a),
            provide(make_b),
        ],
    });
}
