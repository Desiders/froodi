#![allow(dead_code)]

use criterion::{criterion_group, criterion_main, Criterion};
use froodi::{async_impl::Container, async_registry, utils::thread_safety::RcThreadSafety, DefaultScope::*, Inject, InjectTransient};
use tokio::runtime::Builder;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("async_get_single", |b| {
        struct A;

        let container = Container::new(async_registry! {
            scope(App) [
                provide(async || Ok(A)),
            ],
        });
        b.to_async(Builder::new_current_thread().build().unwrap()).iter(|| {
            let container = container.clone();
            async move { container.get::<A>().await.unwrap() }
        });
    })
    .bench_function("async_get_many", |b| {
        struct A(RcThreadSafety<B>, RcThreadSafety<C>);
        struct B(i32);
        struct C(RcThreadSafety<CA>);
        struct CA(RcThreadSafety<CAA>);
        struct CAA(RcThreadSafety<CAAA>);
        struct CAAA(RcThreadSafety<CAAAA>);
        struct CAAAA(RcThreadSafety<CAAAAA>);
        struct CAAAAA;

        let container = Container::new(async_registry! {
            scope(Runtime) [
                provide(async || Ok(CAAAAA)),
            ],
            scope(App) [
                provide(async |Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa))),
            ],
            scope(Session) [
                provide(async |Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa))),
            ],
            scope(Request) [
                provide(async |Inject(caaa): Inject<CAAA>| Ok(CAA(caaa))),
                provide(async |Inject(caa): Inject<CAA>| Ok(CA(caa))),
            ],
            scope(Action) [
                provide(async |Inject(ca): Inject<CA>| Ok(C(ca))),
                provide(async || Ok(B(2))),
            ],
            scope(Step) [
                provide(async |Inject(b): Inject<B>, Inject(c): Inject<C>| Ok(A(b, c))),
            ],
        });
        let scope_container = container.enter().with_scope(Step).build().unwrap();
        b.to_async(Builder::new_current_thread().build().unwrap()).iter(|| {
            let scope_container = scope_container.clone();
            async move { scope_container.get::<A>().await.unwrap() }
        });
    })
    .bench_function("async_get_transient_single", |b| {
        struct A;

        let container = Container::new(async_registry! {
            scope(App) [
                provide(async || Ok(A)),
            ],
        });
        b.to_async(Builder::new_current_thread().build().unwrap()).iter(|| {
            let container = container.clone();
            async move { container.get_transient::<A>().await.unwrap() }
        });
    })
    .bench_function("async_get_transient_many", |b| {
        struct A(B, C);
        struct B(i32);
        struct C(CA);
        struct CA(CAA);
        struct CAA(CAAA);
        struct CAAA(CAAAA);
        struct CAAAA(CAAAAA);
        struct CAAAAA;

        let container = Container::new(async_registry! {
            scope(Runtime) [
                provide(async || Ok(CAAAAA)),
            ],
            scope(App) [
                provide(async |InjectTransient(caaaaa): InjectTransient<CAAAAA>| Ok(CAAAA(caaaaa))),
            ],
            scope(Session) [
                provide(async |InjectTransient(caaaa): InjectTransient<CAAAA>| Ok(CAAA(caaaa))),
            ],
            scope(Request) [
                provide(async |InjectTransient(caaa): InjectTransient<CAAA>| Ok(CAA(caaa))),
                provide(async |InjectTransient(caa): InjectTransient<CAA>| Ok(CA(caa))),
            ],
            scope(Action) [
                provide(async |InjectTransient(ca): InjectTransient<CA>| Ok(C(ca))),
                provide(async || Ok(B(2))),
            ],
            scope(Step) [
                provide(async |InjectTransient(b): InjectTransient<B>, InjectTransient(c): InjectTransient<C>| Ok(A(b, c))),
            ],
        });
        let scope_container = container.enter().with_scope(Step).build().unwrap();
        b.to_async(Builder::new_current_thread().build().unwrap()).iter(|| {
            let scope_container = scope_container.clone();
            async move { scope_container.get_transient::<A>().await.unwrap() }
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
