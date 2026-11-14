//! A shared scope cursor must preserve sibling isolation and skipped-parent cleanup.
use froodi::{
    declare, registry,
    utils::thread_safety::RcThreadSafety,
    Container, Context,
    DefaultScope::{App, Request, Session},
    Inject,
};
use std::sync::Mutex;

struct AppValue;
struct SessionValue;
struct Label(&'static str);
struct RequestValue(&'static str);

#[test]
fn sibling_requests_keep_context_and_skipped_parent_cleanup_independent() {
    let events = RcThreadSafety::new(Mutex::new(Vec::new()));
    let (app_events, session_events, request_events) = (events.clone(), events.clone(), events.clone());
    let app = Container::new(registry! {
        provide(App, || Ok(AppValue), finalizer = move |_| app_events.lock().unwrap().push("app")),
        provide(Session, |_: Inject<AppValue>| Ok(SessionValue), finalizer = move |_| session_events.lock().unwrap().push("session")),
        provide(Request, declare::<Label>()),
        provide(Request, |_: Inject<SessionValue>, Inject(label): Inject<Label>| Ok(RequestValue(label.0)),
            finalizer = move |value: RcThreadSafety<RequestValue>| request_events.lock().unwrap().push(value.0)),
    });
    let mut context = Context::new();
    context.insert(Label("left"));
    let left = app.clone().enter().with_context(context).build().unwrap();
    let mut context = Context::new();
    context.insert(Label("right"));
    let right = app.clone().enter().with_context(context).build().unwrap();
    assert_eq!(left.get::<RequestValue>().unwrap().0, "left");
    assert_eq!(right.get::<RequestValue>().unwrap().0, "right");
    left.close();
    assert_eq!(*events.lock().unwrap(), ["left", "session"]);
    // Closing one request leaves its sibling and the app scope alive.
    assert_eq!(right.get::<RequestValue>().unwrap().0, "right");
    right.close();
    assert_eq!(*events.lock().unwrap(), ["left", "session", "right", "session"]);
    app.close();
    assert_eq!(*events.lock().unwrap(), ["left", "session", "right", "session", "app"]);
}

#[cfg(feature = "async")]
#[tokio::test(flavor = "current_thread")]
async fn async_sibling_requests_preserve_context_and_skipped_parent_cleanup() {
    use froodi::async_impl::Container as AsyncContainer;
    let events = RcThreadSafety::new(Mutex::new(Vec::new()));
    let (app_events, session_events, request_events) = (events.clone(), events.clone(), events.clone());
    let app = AsyncContainer::new(registry! {
        provide(App, async || Ok(AppValue), finalizer = move |_| {
            let events = app_events.clone();
            async move { events.lock().unwrap().push("app"); }
        }),
        provide(Session, async |_: Inject<AppValue>| Ok(SessionValue), finalizer = move |_| {
            let events = session_events.clone();
            async move { events.lock().unwrap().push("session"); }
        }),
        provide(Request, declare::<Label>()),
        provide(Request, async |_: Inject<SessionValue>, Inject(label): Inject<Label>| Ok(RequestValue(label.0)),
            finalizer = move |value: RcThreadSafety<RequestValue>| {
                let events = request_events.clone();
                async move { events.lock().unwrap().push(value.0); }
            }),
    });
    let mut context = Context::new();
    context.insert(Label("left"));
    let left = app.clone().enter().with_context(context).build().unwrap();
    let mut context = Context::new();
    context.insert(Label("right"));
    let right = app.clone().enter().with_context(context).build().unwrap();
    assert_eq!(left.get::<RequestValue>().await.unwrap().0, "left");
    assert_eq!(right.get::<RequestValue>().await.unwrap().0, "right");
    left.close().await;
    assert_eq!(*events.lock().unwrap(), ["left", "session"]);
    assert_eq!(right.get::<RequestValue>().await.unwrap().0, "right");
    right.close().await;
    assert_eq!(*events.lock().unwrap(), ["left", "session", "right", "session"]);
    app.close().await;
    assert_eq!(*events.lock().unwrap(), ["left", "session", "right", "session", "app"]);
}
