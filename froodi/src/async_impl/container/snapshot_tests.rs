//! Both synchronous caches inside an async container follow the same snapshot protocol.
extern crate std;
use super::Container;
use crate::{
    registry, Context,
    DefaultScope::{App, Runtime},
    TypeInfo,
};
use std::{
    sync::{mpsc, Arc},
    thread,
    time::Duration,
};

struct Value(u64);
fn parent() -> (Container, Arc<Value>) {
    let value = Arc::new(Value(7));
    let mut context = Context::new();
    context.insert_rc(value.clone());
    let parent = Container::new_with_start_scope(registry! {}, Runtime)
        .enter()
        .with_scope(App)
        .with_context(context)
        .build()
        .unwrap();
    (parent, value)
}

#[test]
fn async_container_snapshots_share_existing_readers_in_both_caches() {
    let (parent, expected) = parent();
    let async_reader = parent.inner.cache.read();
    let sync_reader = parent.sync.inner.cache.read();
    let worker_parent = parent.clone();
    let (send, receive) = mpsc::channel();
    let worker = thread::spawn(move || {
        let request = worker_parent.enter_build().unwrap();
        let asynchronous = request.inner.cache.read().get::<Value>(&TypeInfo::of::<Value>()).unwrap();
        let synchronous = request.sync.get::<Value>().unwrap();
        send.send((
            asynchronous.0,
            Arc::ptr_eq(&expected, &asynchronous),
            Arc::ptr_eq(&expected, &synchronous),
        ))
        .unwrap();
    });
    let completed = receive.recv_timeout(Duration::from_secs(10));
    drop(sync_reader);
    drop(async_reader);
    worker.join().unwrap();
    assert_eq!(
        completed.expect("one cache snapshot waited for an existing reader"),
        (7, true, true)
    );
}

#[test]
fn async_descendant_and_embedded_sync_cache_have_independent_parent_gates() {
    let (parent, expected) = parent();
    let request = parent.clone().enter_build().unwrap();
    let async_gate = parent.inner.snapshot_gate.write();
    let sync_gate = parent.sync.inner.snapshot_gate.write();
    let (send, receive) = mpsc::channel();
    let worker = thread::spawn(move || {
        let action = request.enter_build().unwrap();
        let asynchronous = action.inner.cache.read().get::<Value>(&TypeInfo::of::<Value>()).unwrap();
        let synchronous = action.sync.get::<Value>().unwrap();
        send.send((Arc::ptr_eq(&expected, &asynchronous), Arc::ptr_eq(&expected, &synchronous)))
            .unwrap();
    });
    let completed = receive.recv_timeout(Duration::from_secs(10));
    drop(sync_gate);
    drop(async_gate);
    worker.join().unwrap();
    assert_eq!(completed.expect("an async descendant reused a parent cache gate"), (true, true));
}
