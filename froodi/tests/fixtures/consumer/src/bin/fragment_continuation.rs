use di::{registry, Container};

fn main() {
    let _ = Container::new(registry! {
        __fragment_placeholder(),
    });
}
