use froodi::{Container, DefaultScope::App};

#[derive(froodi::Construct)]
struct Service {
    #[di(inject_transient)]
    request_id: u32,
}

fn main() {
    let _ = Container::new(froodi::registry! { scope(App) [ provide::<Service>() ] });
}
