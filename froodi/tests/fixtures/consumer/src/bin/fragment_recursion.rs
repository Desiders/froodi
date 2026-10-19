use di::{fragment, registry, Container};
#[fragment(repeated)]
registry! { extend_fragment(repeated!()), }
fn main() {
    let _ = Container::new(registry! { extend_fragment(repeated!()), });
}
