use di::{fragment, instance, registry, Container, DefaultScope::App};

#[fragment(data(value))]
registry! { provide(App, instance(value)), }

fn main() {
    let _ = Container::new(registry! { extend_fragment(data!()), });
}
