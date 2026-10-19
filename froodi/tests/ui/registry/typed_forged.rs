use froodi::{registry, Container, TypedContainer};
use std::marker::PhantomData;

fn main() {
    let _ = TypedContainer::<()> {
        inner: Container::new(registry!()),
        marker: PhantomData,
    };
}
