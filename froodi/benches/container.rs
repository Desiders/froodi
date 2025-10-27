#![allow(dead_code)]

use criterion::{criterion_group, criterion_main, Criterion};
use froodi::{registry, utils::thread_safety::RcThreadSafety, Container, DefaultScope::*, Inject, InjectTransient};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("container_new", |b| {
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
    .bench_function("container_child_start_scope", |b| {
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
    .bench_function("container_child_next", |b| {
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
    })
    .bench_function("container_get", |b| {
        struct A(RcThreadSafety<B>, RcThreadSafety<C>);
        struct B(i32);
        struct C(RcThreadSafety<CA>);
        struct CA(RcThreadSafety<CAA>);
        struct CAA(RcThreadSafety<CAAA>);
        struct CAAA(RcThreadSafety<CAAAA>);
        struct CAAAA(RcThreadSafety<CAAAAA>);
        struct CAAAAA;

        let container = Container::new(registry! {
            scope(Runtime) [
                provide(|| Ok(CAAAAA)),
            ],
            scope(App) [
                provide(|Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa))),
            ],
            scope(Session) [
                provide(|Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa))),
            ],
            scope(Request) [
                provide(|Inject(caaa): Inject<CAAA>| Ok(CAA(caaa))),
                provide(|Inject(caa): Inject<CAA>| Ok(CA(caa))),
            ],
            scope(Action) [
                provide(|Inject(ca): Inject<CA>| Ok(C(ca))),
                provide(|| Ok(B(2))),
            ],
            scope(Step) [
                provide(|Inject(b): Inject<B>, Inject(c): Inject<C>| Ok(A(b, c))),
            ],
        });
        let scope_container = container.enter().with_scope(Step).build().unwrap();
        b.iter(|| scope_container.get::<A>().unwrap());
    })
    .bench_function("container_get_transient", |b| {
        struct A(B, C);
        struct B(i32);
        struct C(CA);
        struct CA(CAA);
        struct CAA(CAAA);
        struct CAAA(CAAAA);
        struct CAAAA(CAAAAA);
        struct CAAAAA;

        let container = Container::new(registry! {
            scope(Runtime) [
                provide(|| Ok(CAAAAA)),
            ],
            scope(App) [
                provide(|InjectTransient(caaaaa): InjectTransient<CAAAAA>| Ok(CAAAA(caaaaa))),
            ],
            scope(Session) [
                provide(|InjectTransient(caaaa): InjectTransient<CAAAA>| Ok(CAAA(caaaa))),
            ],
            scope(Request) [
                provide(|InjectTransient(caaa): InjectTransient<CAAA>| Ok(CAA(caaa))),
                provide(|InjectTransient(caa): InjectTransient<CAA>| Ok(CA(caa))),
            ],
            scope(Action) [
                provide(|InjectTransient(ca): InjectTransient<CA>| Ok(C(ca))),
                provide(|| Ok(B(2))),
            ],
            scope(Step) [
                provide(
                    |InjectTransient(b): InjectTransient<B>, InjectTransient(c): InjectTransient<C>| Ok(A(b, c)),
                ),
            ],
        });
        let scope_container = container.enter().with_scope(Step).build().unwrap();
        b.iter(|| scope_container.get::<A>().unwrap());
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
