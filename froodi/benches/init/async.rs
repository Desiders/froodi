#![allow(dead_code)]

use criterion::{criterion_group, criterion_main, Criterion};
use froodi::{async_impl::Container, async_registry, utils::thread_safety::RcThreadSafety, DefaultScope::*};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("async_new", |b| {
        b.iter(|| {
            Container::new(async_registry! {
                scope(Runtime) [
                    provide(async || Ok(()), finalizer = |_: RcThreadSafety<()>| async {}),
                ],
                scope(App) [
                    provide(async || Ok(((), ())), finalizer = |_: RcThreadSafety<((), ())>| async {}),
                ],
                scope(Session) [
                    provide(async || Ok(((), (), ())), finalizer = |_: RcThreadSafety<((), (), ())>| async {}),
                ],
                scope(Request) [
                    provide(async || Ok(((), (), (), ())), finalizer = |_: RcThreadSafety<((), (), (), ())>| async {}),
                ],
                scope(Action) [
                    provide(async || Ok(((), (), (), (), ())), finalizer = |_: RcThreadSafety<((), (), (), (), ())>| async {}),
                ],
                scope(Step) [
                    provide(async || Ok(((), (), (), (), (), ())), finalizer = |_: RcThreadSafety<((), (), (), (), (), ())>| async {}),
                ],
            })
        });
    })
    .bench_function("async_child_start_scope", |b| {
        let runtime_container = Container::new_with_start_scope(
            async_registry! {
                scope(Runtime) [
                    provide(async || Ok(())),
                ],
                scope(App) [
                    provide(async || Ok(((), ()))),
                ],
                scope(Session) [
                    provide(async || Ok(((), (), ()))),
                ],
                scope(Request) [
                    provide(async || Ok(((), (), (), ()))),
                ],
                scope(Action) [
                    provide(async || Ok(((), (), (), (), ()))),
                ],
                scope(Step) [
                    provide(async || Ok(((), (), (), (), (), ()))),
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
    .bench_function("async_child_next", |b| {
        let app_container = Container::new(async_registry! {
            scope(Runtime) [
                provide(async || Ok(())),
            ],
            scope(App) [
                provide(async || Ok(((), ()))),
            ],
            scope(Session) [
                provide(async || Ok(((), (), ()))),
            ],
            scope(Request) [
                provide(async || Ok(((), (), (), ()))),
            ],
            scope(Action) [
                provide(async || Ok(((), (), (), (), ()))),
            ],
            scope(Step) [
                provide(async || Ok(((), (), (), (), (), ()))),
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
