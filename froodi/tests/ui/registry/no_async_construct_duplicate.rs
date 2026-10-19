use froodi::{Container, DefaultScope::App, Inject, InstantiateErrorKind};

#[derive(froodi::Construct)]
struct Service;

fn main() {
    let _ = Container::new(froodi::registry! {
        scope(App) [ provide::<Service>() ],
        provide(App, || Ok::<_, InstantiateErrorKind>(Service)),
        provide(App, |_: Inject<Service>| Ok::<_, InstantiateErrorKind>(7u32)),
    });
}
