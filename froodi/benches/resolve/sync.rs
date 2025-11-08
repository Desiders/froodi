#![allow(dead_code)]

use criterion::{criterion_group, criterion_main, Criterion};
use froodi::{registry, utils::thread_safety::RcThreadSafety, Container, DefaultScope::*, Inject, InjectTransient};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("sync_get_single", |b| {
        struct A;

        let container = Container::new(registry! {
            scope(App) [
                provide(|| Ok(A)),
            ],
        });
        b.iter(|| container.get::<A>().unwrap());
    })
    .bench_function("sync_get_many", |b| {
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
    .bench_function("sync_get_transient_single", |b| {
        struct A;

        let container = Container::new(registry! {
            scope(App) [
                provide(|| Ok(A)),
            ],
        });
        b.iter(|| container.get_transient::<A>().unwrap());
    })
    .bench_function("sync_get_transient_many", |b| {
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
        b.iter(|| scope_container.get_transient::<A>().unwrap());
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
