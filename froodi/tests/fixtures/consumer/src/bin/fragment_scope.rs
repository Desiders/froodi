use di::{fragment, instance, registry, Container, Inject, InstantiateErrorKind};
use scope_support::scopes;

#[fragment(first)]
registry! { provide(scopes::App, |_: Inject<u32>| Ok::<_, InstantiateErrorKind>(true)), }
#[fragment(second)]
registry! { provide(scopes::Request, instance(7u32)), }

fn main() {
    let _ = Container::new(registry! { extend_fragment(first!(), second!()), });
}
