#![allow(dead_code)]

use criterion::{criterion_group, criterion_main, Criterion};
use froodi::{
    async_impl::{Container, RegistryBuilder},
    utils::thread_safety::RcThreadSafety,
    DefaultScope::*,
    Inject, InjectTransient,
};
use tokio::runtime::Builder;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("container_new_with_registry_builder", |b| {
        b.iter(|| {
            Container::new(
                RegistryBuilder::new()
                    .provide_async(async || Ok(()), Runtime)
                    .provide_async(async || Ok(((), ())), App)
                    .provide_async(async || Ok(((), (), ())), Session)
                    .provide_async(async || Ok(((), (), (), ())), Request)
                    .provide_async(async || Ok(((), (), (), (), ())), Action)
                    .provide_async(async || Ok(((), (), (), (), (), ())), Step)
                    .add_async_finalizer(|_: RcThreadSafety<()>| async {})
                    .add_async_finalizer(|_: RcThreadSafety<((), ())>| async {})
                    .add_async_finalizer(|_: RcThreadSafety<((), (), ())>| async {})
                    .add_async_finalizer(|_: RcThreadSafety<((), (), (), ())>| async {})
                    .add_async_finalizer(|_: RcThreadSafety<((), (), (), (), ())>| async {})
                    .add_async_finalizer(|_: RcThreadSafety<((), (), (), (), (), ())>| async {}),
            )
        });
    })
    .bench_function("container_child_with_scope", |b| {
        let runtime_container = Container::new_with_start_scope(
            RegistryBuilder::new()
                .provide_async(async || Ok(()), Runtime)
                .provide_async(async || Ok(((), ())), App)
                .provide_async(async || Ok(((), (), ())), Session)
                .provide_async(async || Ok(((), (), (), ())), Request)
                .provide_async(async || Ok(((), (), (), (), ())), Action)
                .provide_async(async || Ok(((), (), (), (), (), ())), Step),
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
    .bench_function("container_child_with_hierarchy", |b| {
        let app_container = Container::new(
            RegistryBuilder::new()
                .provide_async(async || Ok(()), Runtime)
                .provide_async(async || Ok(((), ())), App)
                .provide_async(async || Ok(((), (), ())), Session)
                .provide_async(async || Ok(((), (), (), ())), Request)
                .provide_async(async || Ok(((), (), (), (), ())), Action)
                .provide_async(async || Ok(((), (), (), (), (), ())), Step),
        );
        b.iter(|| {
            let request_container = app_container.clone().enter_build().unwrap();
            let action_container = request_container.enter_build().unwrap();
            let _ = action_container.enter_build().unwrap();
        });
    })
    .bench_function("container_get_with_cache", |b| {
        struct A(RcThreadSafety<B>, RcThreadSafety<C>);
        struct B(i32);
        struct C(RcThreadSafety<CA>);
        struct CA(RcThreadSafety<CAA>);
        struct CAA(RcThreadSafety<CAAA>);
        struct CAAA(RcThreadSafety<CAAAA>);
        struct CAAAA(RcThreadSafety<CAAAAA>);
        struct CAAAAA;

        let container = Container::new(
            RegistryBuilder::new()
                .provide_async(async || (Ok(CAAAAA)), Request)
                .provide_async(async |Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa)), Request)
                .provide_async(async |Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa)), Request)
                .provide_async(async |Inject(caaa): Inject<CAAA>| Ok(CAA(caaa)), Request)
                .provide_async(async |Inject(caa): Inject<CAA>| Ok(CA(caa)), Request)
                .provide_async(async |Inject(ca): Inject<CA>| Ok(C(ca)), Request)
                .provide_async(async || Ok(B(2)), Request)
                .provide_async(async |Inject(b): Inject<B>, Inject(c): Inject<C>| Ok(A(b, c)), Request),
        );
        let request_container = container.enter_build().unwrap();
        b.to_async(Builder::new_current_thread().build().unwrap()).iter(|| {
            let request_container = request_container.clone();
            async move { request_container.get::<A>().await.unwrap() }
        });
    })
    .bench_function("container_get_with_hierarchy_and_cache", |b| {
        struct A(RcThreadSafety<B>, RcThreadSafety<C>);
        struct B(i32);
        struct C(RcThreadSafety<CA>);
        struct CA(RcThreadSafety<CAA>);
        struct CAA(RcThreadSafety<CAAA>);
        struct CAAA(RcThreadSafety<CAAAA>);
        struct CAAAA(RcThreadSafety<CAAAAA>);
        struct CAAAAA;

        let container = Container::new(
            RegistryBuilder::new()
                .provide_async(async || (Ok(CAAAAA)), Runtime)
                .provide_async(async |Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa)), App)
                .provide_async(async |Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa)), Session)
                .provide_async(async |Inject(caaa): Inject<CAAA>| Ok(CAA(caaa)), Request)
                .provide_async(async |Inject(caa): Inject<CAA>| Ok(CA(caa)), Request)
                .provide_async(async |Inject(ca): Inject<CA>| Ok(C(ca)), Action)
                .provide_async(async || Ok(B(2)), Action)
                .provide_async(async |Inject(b): Inject<B>, Inject(c): Inject<C>| Ok(A(b, c)), Step),
        );
        let scope_container = container.enter().with_scope(Step).build().unwrap();
        b.to_async(Builder::new_current_thread().build().unwrap()).iter(|| {
            let scope_container = scope_container.clone();
            async move { scope_container.get::<A>().await.unwrap() }
        });
    })
    .bench_function("container_get_without_cache", |b| {
        struct A(B, C);
        struct B(i32);
        struct C(CA);
        struct CA(CAA);
        struct CAA(CAAA);
        struct CAAA(CAAAA);
        struct CAAAA(CAAAAA);
        struct CAAAAA;

        let container = Container::new(
            RegistryBuilder::new()
                .provide_async(async || (Ok(CAAAAA)), Request)
                .provide_async(async |InjectTransient(caaaaa): InjectTransient<CAAAAA>| Ok(CAAAA(caaaaa)), Request)
                .provide_async(async |InjectTransient(caaaa): InjectTransient<CAAAA>| Ok(CAAA(caaaa)), Request)
                .provide_async(async |InjectTransient(caaa): InjectTransient<CAAA>| Ok(CAA(caaa)), Request)
                .provide_async(async |InjectTransient(caa): InjectTransient<CAA>| Ok(CA(caa)), Request)
                .provide_async(async |InjectTransient(ca): InjectTransient<CA>| Ok(C(ca)), Request)
                .provide_async(async || Ok(B(2)), Request)
                .provide_async(
                    async |InjectTransient(b): InjectTransient<B>, InjectTransient(c): InjectTransient<C>| Ok(A(b, c)),
                    Request,
                ),
        );
        let request_container = container.enter_build().unwrap();
        b.to_async(Builder::new_current_thread().build().unwrap()).iter(|| {
            let request_container = request_container.clone();
            async move { request_container.get::<A>().await.unwrap() }
        });
    })
    .bench_function("container_get_with_hierarchy_without_cache", |b| {
        struct A(B, C);
        struct B(i32);
        struct C(CA);
        struct CA(CAA);
        struct CAA(CAAA);
        struct CAAA(CAAAA);
        struct CAAAA(CAAAAA);
        struct CAAAAA;

        let container = Container::new(
            RegistryBuilder::new()
                .provide_async(async || (Ok(CAAAAA)), Runtime)
                .provide_async(async |InjectTransient(caaaaa): InjectTransient<CAAAAA>| Ok(CAAAA(caaaaa)), App)
                .provide_async(async |InjectTransient(caaaa): InjectTransient<CAAAA>| Ok(CAAA(caaaa)), Session)
                .provide_async(async |InjectTransient(caaa): InjectTransient<CAAA>| Ok(CAA(caaa)), Request)
                .provide_async(async |InjectTransient(caa): InjectTransient<CAA>| Ok(CA(caa)), Request)
                .provide_async(async |InjectTransient(ca): InjectTransient<CA>| Ok(C(ca)), Action)
                .provide_async(async || Ok(B(2)), Action)
                .provide_async(
                    async |InjectTransient(b): InjectTransient<B>, InjectTransient(c): InjectTransient<C>| Ok(A(b, c)),
                    Step,
                ),
        );
        let scope_container = container.enter().with_scope(Step).build().unwrap();
        b.to_async(Builder::new_current_thread().build().unwrap()).iter(|| {
            let scope_container = scope_container.clone();
            async move { scope_container.get::<A>().await.unwrap() }
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
