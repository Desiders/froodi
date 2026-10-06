use froodi::{Container, DefaultScope::App, InstantiateErrorKind};

#[derive(froodi::Construct)]
struct Service {
    #[di(inject_transient)]
    request_id: u32,
}

fn main() {
    let _ = Container::new(froodi::registry! {
        scope(App) [ provide::<Service>() ],
        provide(App, async || Ok::<_, InstantiateErrorKind>(7u32)),
    });
}
