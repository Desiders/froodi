//! Type identity belongs to rustc (issue #57, ADR 0003): spellings that name the same type are
//! the same binding, and no string comparison is involved.

#![allow(clippy::unnecessary_wraps, reason = "factories return Result by contract")]

use froodi_compile::{registry, Container, DefaultScope::*, Inject, InstantiateErrorKind};

mod settings {
    pub struct Config(pub u8);
}

type Alias = settings::Config;

struct Consumer(u8);

fn make_consumer(Inject(config): Inject<crate::settings::Config>) -> Result<Consumer, InstantiateErrorKind> {
    Ok(Consumer(config.0))
}

#[test]
fn an_alias_and_a_full_path_name_the_same_binding() {
    let container = Container::new(registry! {
        scope(App) [
            provide(|| Ok::<Alias, InstantiateErrorKind>(Alias { 0: 7 })),
            provide(make_consumer),
        ],
    });

    assert_eq!(container.get::<Consumer>().unwrap().0, 7);
    assert_eq!(container.get::<settings::Config>().unwrap().0, 7);
}
