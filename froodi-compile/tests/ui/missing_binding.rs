use froodi_compile::{registry, Container, DefaultScope::*, Inject, InstantiateErrorKind};

struct Config;
struct Database;

fn make_database(Inject(_config): Inject<Config>) -> Result<Database, InstantiateErrorKind> {
    Ok(Database)
}

fn main() {
    let _container = Container::new(registry! {
        scope(App) [
            provide(make_database),
        ],
    });
}
