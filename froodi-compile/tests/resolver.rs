//! Custom `DependencyResolver` parameters: user code that reads the container at runtime.

#![allow(clippy::unnecessary_wraps, reason = "factories return Result by contract")]

use froodi_compile::{ir::RequestMode, registry, Container, DefaultScope::*, DependencyResolver, InstantiateErrorKind, ResolveErrorKind};

struct Counter(u64);
/// Reads another registration through the public container API, like Froodi's `MapInject`.
struct DoubledCounter(u64);

impl DependencyResolver for DoubledCounter {
    type Error = ResolveErrorKind;

    fn resolve(container: &Container) -> Result<Self, Self::Error> {
        Ok(Self(container.get::<Counter>()?.0 * 2))
    }
}

struct Report(u64);

fn make_report(DoubledCounter(value): DoubledCounter) -> Result<Report, InstantiateErrorKind> {
    Ok(Report(value))
}

macro_rules! report_registry {
    () => {
        registry! {
            scope(App) [
                provide(|| Ok::<_, InstantiateErrorKind>(Counter(21))),
                provide(make_report),
            ],
        }
    };
}

#[test]
fn resolves_a_custom_resolver_parameter() {
    let container = Container::new(report_registry!());

    assert_eq!(container.get::<Report>().unwrap().0, 42);
}

#[test]
fn records_a_custom_resolver_as_an_opaque_request() {
    let graph = report_registry!().graph();

    assert_eq!(graph.registrations[1].requests[0].mode, RequestMode::Resolver);
}
