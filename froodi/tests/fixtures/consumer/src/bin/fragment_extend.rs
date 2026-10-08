use di::{fragment, instance, registry, Container, DefaultScope::App};

#[fragment(configuration(value))]
registry! {
    provide(App, instance(value)),
}

fn main() {
    let _ = Container::new(registry! { extend(configuration!(7u32)), });
}
