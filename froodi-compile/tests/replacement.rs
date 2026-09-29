//! Explicit replacement of registrations, for test overrides (issue #62).
#![allow(clippy::unnecessary_wraps, reason = "factories return Result by contract")]

use froodi_compile::{registry, Container, DefaultScope::*, Inject, InjectTransient, InstantiateErrorKind};

#[derive(Clone)]
struct Config(&'static str);
struct Database(&'static str);
struct Snapshot(&'static str);

fn make_config() -> Result<Config, InstantiateErrorKind> {
    Ok(Config("production"))
}

fn make_database(Inject(config): Inject<Config>) -> Result<Database, InstantiateErrorKind> {
    Ok(Database(config.0))
}

fn make_snapshot(InjectTransient(config): InjectTransient<Config>) -> Result<Snapshot, InstantiateErrorKind> {
    Ok(Snapshot(config.0))
}

#[test]
fn a_replacing_registry_overrides_static_registrations_everywhere() {
    let overrides = registry! {
        scope(App) [ provide(|| Ok::<_, InstantiateErrorKind>(Config("test"))) ],
    }
    .into_runtime()
    .replacing();

    let container = Container::new(registry! {
        scope(App) [
            provide(make_config),
            provide(make_database),
            provide(make_snapshot),
        ],
        extend(overrides),
    });

    assert_eq!(container.get::<Config>().unwrap().0, "test");
    assert_eq!(container.get::<Database>().unwrap().0, "test");
    assert_eq!(container.get::<Snapshot>().unwrap().0, "test");
    assert_eq!(container.get_transient::<Config>().unwrap().0, "test");
}
