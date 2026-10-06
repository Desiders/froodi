use di::{fragment, registry, Container};
#[fragment(first)]
registry! { extend_fragment(second!()), }
#[fragment(second)]
registry! { extend_fragment(first!()), }
fn main() {
    let _ = Container::new(registry! { extend_fragment(first!()), });
}
