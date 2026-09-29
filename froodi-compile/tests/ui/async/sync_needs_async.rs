use froodi_compile::{async_impl::Container, async_registry, registry, DefaultScope::*, Inject, InstantiateErrorKind};

struct Config;
struct Database;

fn main() {
    let _container = Container::new(async_registry! {
        scope(App) [ provide(async || Ok::<_, InstantiateErrorKind>(Config)) ],
        extend(registry! {
            scope(App) [ provide(|Inject(_config): Inject<Config>| Ok::<_, InstantiateErrorKind>(Database)) ],
        }),
    });
}
