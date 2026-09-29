use froodi_compile::{registry, Container, DefaultScope::*, Inject, InstantiateErrorKind};

struct Plugin;
struct Host;

fn make_host(Inject(_plugin): Inject<Plugin>) -> Result<Host, InstantiateErrorKind> {
    Ok(Host)
}

fn main() {
    let plugins = registry! {
        scope(App) [ provide(|| Ok::<_, InstantiateErrorKind>(Plugin)) ],
    }
    .into_runtime();
    let _container = Container::new(registry! {
        scope(App) [ provide(make_host) ],
        extend(plugins),
    });
}
