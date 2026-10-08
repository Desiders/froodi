use di::{registry, Container};

fn main() {
    let _ = Container::new(registry! { extend_fragment(no_such_fragment!()), });
}
