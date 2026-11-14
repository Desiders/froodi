//! Error exits must preserve cleanup order while consuming the parent and Context.
use froodi::{
    registry,
    utils::thread_safety::RcThreadSafety,
    Container, Context,
    DefaultScope::{App, Runtime, Step},
};
use std::sync::Mutex;

type Events = RcThreadSafety<Mutex<Vec<&'static str>>>;
struct Resource;
struct ContextDrop(Events);
impl Drop for ContextDrop {
    fn drop(&mut self) {
        self.0.lock().unwrap().push("context");
    }
}

#[test]
fn failed_sync_scope_build_keeps_parent_finalizer_before_context_drop() {
    for no_children in [true, false] {
        let events = RcThreadSafety::new(Mutex::new(Vec::new()));
        let finalized = events.clone();
        let scope = if no_children { Step } else { App };
        let parent = Container::new_with_start_scope(
            registry! {
                provide(scope, || Ok(Resource), finalizer = move |_| finalized.lock().unwrap().push("finalizer")),
            },
            scope,
        );
        drop(parent.get::<Resource>().unwrap());
        let mut context = Context::new();
        context.insert(ContextDrop(events.clone()));
        let failed = if no_children {
            parent.enter().with_context(context).build().is_err()
        } else {
            parent.enter().with_context(context).with_scope(Runtime).build().is_err()
        };
        assert!(failed);
        assert_eq!(*events.lock().unwrap(), ["finalizer", "context"], "no_children={no_children}");
    }
}

#[cfg(feature = "async")]
#[tokio::test(flavor = "current_thread")]
async fn failed_async_scope_build_keeps_sync_finalizer_before_context_drop() {
    use froodi::async_impl::Container as AsyncContainer;
    for no_children in [true, false] {
        let events = RcThreadSafety::new(Mutex::new(Vec::new()));
        let finalized = events.clone();
        let scope = if no_children { Step } else { App };
        // An async container still owns synchronous providers and their cleanup.
        let parent = AsyncContainer::new_with_start_scope(
            registry! {
                provide(scope, || Ok(Resource), finalizer = move |_| finalized.lock().unwrap().push("finalizer")),
            },
            scope,
        );
        drop(parent.get::<Resource>().await.unwrap());
        let mut context = Context::new();
        context.insert(ContextDrop(events.clone()));
        let failed = if no_children {
            parent.enter().with_context(context).build().is_err()
        } else {
            parent.enter().with_context(context).with_scope(Runtime).build().is_err()
        };
        assert!(failed);
        assert_eq!(*events.lock().unwrap(), ["finalizer", "context"], "no_children={no_children}");
    }
}
