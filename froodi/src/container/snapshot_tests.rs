//! Synchronization protocol checks. Timeouts are deadlock watchdogs, never performance assertions.
extern crate std;
use super::Container;
use crate::{
    registry, Context,
    DefaultScope::{App, Runtime},
    TypeInfo,
};
use std::{
    sync::{mpsc, Arc, Barrier},
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
fn snapshot_can_complete_while_an_existing_cache_reader_is_held() {
    let (parent, expected) = parent();
    let existing_reader = parent.inner.cache.read();
    let worker_parent = parent.clone();
    let (send, receive) = mpsc::channel();
    let worker = thread::spawn(move || {
        let child = worker_parent.enter_build().unwrap();
        let actual = child.get::<Value>().unwrap();
        send.send((actual.0, Arc::ptr_eq(&actual, &expected))).unwrap();
    });
    let completed = receive.recv_timeout(Duration::from_secs(10));
    // Always release the held guard before joining, including a failed/deadlocked protocol.
    drop(existing_reader);
    worker.join().unwrap();
    assert_eq!(completed.expect("snapshot blocked an existing reader"), (7, true));
}

#[test]
fn ordinary_cache_read_and_write_do_not_wait_for_the_snapshot_gate() {
    let (parent, expected) = parent();
    let gate = parent.inner.snapshot_gate.write();
    let worker_parent = parent.clone();
    let replacement = Arc::new(Value(9));
    let (send, receive) = mpsc::channel();
    let worker = thread::spawn(move || {
        let first = worker_parent.get::<Value>().unwrap();
        let previous = worker_parent
            .inner
            .cache
            .write()
            .insert_rc(TypeInfo::of::<Value>(), replacement.clone());
        let second = worker_parent.get::<Value>().unwrap();
        send.send((first.0, previous.unwrap().0, Arc::ptr_eq(&replacement, &second)))
            .unwrap();
    });
    let completed = receive.recv_timeout(Duration::from_secs(10));
    drop(gate);
    worker.join().unwrap();
    assert_eq!(completed.expect("ordinary cache operations took the snapshot gate"), (7, 7, true));
    // A normal close still restores the original Context map after the controlled write.
    parent.close();
    assert!(Arc::ptr_eq(&expected, &parent.get::<Value>().unwrap()));
}

#[test]
fn a_descendant_uses_its_own_gate_instead_of_its_parents_gate() {
    let (parent, expected) = parent();
    let request = parent.clone().enter_build().unwrap();
    let gate = parent.inner.snapshot_gate.write();
    let (send, receive) = mpsc::channel();
    let worker = thread::spawn(move || {
        let action = request.enter_build().unwrap();
        let actual = action.get::<Value>().unwrap();
        send.send(Arc::ptr_eq(&expected, &actual)).unwrap();
    });
    let completed = receive.recv_timeout(Duration::from_secs(10));
    drop(gate);
    worker.join().unwrap();
    assert!(completed.expect("a descendant reused its parent's gate"));
}

#[test]
fn concurrent_snapshots_keep_identity_while_a_writer_replaces_the_parent_value() {
    let (parent, original) = parent();
    let replacement = Arc::new(Value(9));
    let start = Arc::new(Barrier::new(5));
    let mut workers = std::vec::Vec::new();
    for _ in 0..3 {
        let (parent, original, replacement, start) = (parent.clone(), original.clone(), replacement.clone(), start.clone());
        workers.push(thread::spawn(move || {
            start.wait();
            for _ in 0..64 {
                let request = parent.clone().enter_build().unwrap();
                let first = request.get::<Value>().unwrap();
                let cached = request.get::<Value>().unwrap();
                assert!(Arc::ptr_eq(&first, &cached));
                assert!(Arc::ptr_eq(&first, &original) || Arc::ptr_eq(&first, &replacement));
            }
        }));
    }
    let (writer_parent, writer_value, writer_start) = (parent.clone(), replacement.clone(), start.clone());
    workers.push(thread::spawn(move || {
        writer_start.wait();
        for _ in 0..64 {
            let previous = writer_parent
                .inner
                .cache
                .write()
                .insert_rc(TypeInfo::of::<Value>(), writer_value.clone());
            drop(previous);
        }
    }));
    start.wait();
    for worker in workers {
        worker.join().unwrap();
    }
    assert!(Arc::ptr_eq(&replacement, &parent.get::<Value>().unwrap()));
    parent.close();
    assert!(Arc::ptr_eq(&original, &parent.get::<Value>().unwrap()));
}
