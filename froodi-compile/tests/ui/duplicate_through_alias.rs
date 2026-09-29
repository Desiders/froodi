use froodi_compile::{registry, Container, DefaultScope::*, Inject, InstantiateErrorKind};

mod settings {
    pub struct Config;
}

type Alias = settings::Config;

struct Consumer;

fn make_consumer(Inject(_config): Inject<settings::Config>) -> Result<Consumer, InstantiateErrorKind> {
    Ok(Consumer)
}

fn main() {
    let _container = Container::new(registry! {
        scope(App) [
            provide(|| Ok::<settings::Config, InstantiateErrorKind>(settings::Config)),
            provide(|| Ok::<Alias, InstantiateErrorKind>(Alias {})),
            provide(make_consumer),
        ],
    });
}
