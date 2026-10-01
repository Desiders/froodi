use froodi::{utils::thread_safety::RcThreadSafety, DependencyResolver, Inject};
use std::marker::PhantomData;

pub struct Empty;
pub struct Provider<T>(PhantomData<fn() -> T>);
struct ByResolver;
struct LinkedInject;

// A blanket resolver alternative also matches Inject<T>, which implements DependencyResolver.
trait Candidate<Root, Links> {
    const TARGET: Option<usize>;
}

impl<T: DependencyResolver, Root> Candidate<Root, ByResolver> for T {
    const TARGET: Option<usize> = None;
}

impl<T> Candidate<Provider<T>, LinkedInject> for Inject<T> {
    const TARGET: Option<usize> = Some(0);
}

fn infer<Root, T: Candidate<Root, Links>, Links>(_: PhantomData<Root>, _: T) -> Option<usize> {
    T::TARGET
}
