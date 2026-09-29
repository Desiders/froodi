//! `context::<T>()`: a static registration whose value is supplied through `Context`.
#![allow(clippy::unnecessary_wraps, reason = "factories return Result by contract")]

use froodi_compile::{context, registry, Container, Context, DefaultScope::*, Inject, InstantiateErrorKind, ResolveErrorKind};

struct RequestId(u64);
struct Handler(u64);

fn make_handler(Inject(id): Inject<RequestId>) -> Result<Handler, InstantiateErrorKind> {
    Ok(Handler(id.0))
}

fn app() -> Container {
    Container::new(registry! {
        scope(Request) [
            provide(context::<RequestId>()),
            provide(make_handler),
        ],
    })
}

#[test]
fn a_static_factory_depends_on_a_context_value() {
    let mut context = Context::new();
    context.insert(RequestId(42));

    let request = app().enter().with_context(context).build().unwrap();

    assert_eq!(request.get::<Handler>().unwrap().0, 42);
}

#[test]
fn a_missing_context_value_is_a_resolution_error() {
    let request = app().enter_build().unwrap();

    let err = request.get::<Handler>().err().expect("no RequestId in the context");
    assert!(format!("{err:?}").contains("NoContextValue"), "{err:?}");
    assert!(matches!(
        request.get::<RequestId>(),
        Err(ResolveErrorKind::NoContextValue { scope: "request", .. })
    ));
}
