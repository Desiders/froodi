use froodi::{instance, Container, DefaultScope::App};

#[derive(froodi::Construct)]
struct Service {
    #[di(inject_transient)]
    request_id: u32,
}

fn main() {
    let _ = Container::new(froodi::registry! {
        scope(App) [ provide::<Service>() ],
        provide(App, instance(7u32)),
        provide(App, instance(8u32)),
    });
}
