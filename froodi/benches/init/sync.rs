#![allow(dead_code)]

use criterion::{criterion_group, criterion_main, Criterion};
use froodi::{registry, utils::thread_safety::RcThreadSafety, Container, DefaultScope::*};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("sync_new", |b| {
        b.iter(|| {
            Container::new(registry! {
                scope(Runtime) [
                    provide(|| Ok(()), finalizer = |_: RcThreadSafety<()>| {}),
                ],
                scope(App) [
                    provide(|| Ok(((), ())), finalizer = |_: RcThreadSafety<((), ())>| {}),
                ],
                scope(Session) [
                    provide(|| Ok(((), (), ())), finalizer = |_: RcThreadSafety<((), (), ())>| {}),
                ],
                scope(Request) [
                    provide(|| Ok(((), (), (), ())), finalizer = |_: RcThreadSafety<((), (), (), ())>| {}),
                ],
                scope(Action) [
                    provide(|| Ok(((), (), (), (), ())), finalizer = |_: RcThreadSafety<((), (), (), (), ())>| {}),
                ],
                scope(Step) [
                    provide(|| Ok(((), (), (), (), (), ())), finalizer = |_: RcThreadSafety<((), (), (), (), (), ())>| {}),
                ],
            })
        });
    })
    .bench_function("sync_child_start_scope", |b| {
        let runtime_container = Container::new_with_start_scope(
            registry! {
                scope(Runtime) [
                    provide(|| Ok(())),
                ],
                scope(App) [
                    provide(|| Ok(((), ()))),
                ],
                scope(Session) [
                    provide(|| Ok(((), (), ()))),
                ],
                scope(Request) [
                    provide(|| Ok(((), (), (), ()))),
                ],
                scope(Action) [
                    provide(|| Ok(((), (), (), (), ()))),
                ],
                scope(Step) [
                    provide(|| Ok(((), (), (), (), (), ()))),
                ],
            },
            Runtime,
        );
        b.iter(|| {
            let app_container = runtime_container.clone().enter().with_scope(App).build().unwrap();
            let session_container = app_container.enter().with_scope(Session).build().unwrap();
            let request_container = session_container.enter().with_scope(Request).build().unwrap();
            let action_container = request_container.enter().with_scope(Action).build().unwrap();
            let _ = action_container.enter().with_scope(Step).build().unwrap();
        });
    })
    .bench_function("sync_child_next", |b| {
        let app_container = Container::new(registry! {
            scope(Runtime) [
                provide(|| Ok(())),
            ],
            scope(App) [
                provide(|| Ok(((), ()))),
            ],
            scope(Session) [
                provide(|| Ok(((), (), ()))),
            ],
            scope(Request) [
                provide(|| Ok(((), (), (), ()))),
            ],
            scope(Action) [
                provide(|| Ok(((), (), (), (), ()))),
            ],
            scope(Step) [
                provide(|| Ok(((), (), (), (), (), ()))),
            ],
        });
        b.iter(|| {
            let request_container = app_container.clone().enter_build().unwrap();
            let action_container = request_container.enter_build().unwrap();
            let _ = action_container.enter_build().unwrap();
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
