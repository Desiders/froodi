#![allow(dead_code)]

use criterion::{criterion_group, criterion_main, Criterion};
use froodi::{Container, DefaultScope::*, Inject, InjectTransient, RegistriesBuilder};
use std::sync::Arc;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("container_new_with_registries_builder", |b| {
        b.iter(|| {
            Container::new(
                RegistriesBuilder::new()
                    .provide(|| Ok(()), Runtime)
                    .provide(|| Ok(((), ())), App)
                    .provide(|| Ok(((), (), ())), Session)
                    .provide(|| Ok(((), (), (), ())), Request)
                    .provide(|| Ok(((), (), (), (), ())), Action)
                    .provide(|| Ok(((), (), (), (), (), ())), Step)
                    .add_finalizer(|_: Arc<()>| {})
                    .add_finalizer(|_: Arc<((), ())>| {})
                    .add_finalizer(|_: Arc<((), (), ())>| {})
                    .add_finalizer(|_: Arc<((), (), (), ())>| {})
                    .add_finalizer(|_: Arc<((), (), (), (), ())>| {})
                    .add_finalizer(|_: Arc<((), (), (), (), (), ())>| {}),
            )
        })
    })
    .bench_function("container_child_with_scope", |b| {
        let runtime_container = Container::new_with_start_scope(
            RegistriesBuilder::new()
                .provide(|| Ok(()), Runtime)
                .provide(|| Ok(((), ())), App)
                .provide(|| Ok(((), (), ())), Session)
                .provide(|| Ok(((), (), (), ())), Request)
                .provide(|| Ok(((), (), (), (), ())), Action)
                .provide(|| Ok(((), (), (), (), (), ())), Step),
            Runtime,
        );
        b.iter(|| {
            let app_container = runtime_container.clone().enter().with_scope(App).build().unwrap();
            let session_container = app_container.enter().with_scope(Session).build().unwrap();
            let request_container = session_container.enter().with_scope(Request).build().unwrap();
            let action_container = request_container.enter().with_scope(Action).build().unwrap();
            let _ = action_container.enter().with_scope(Step).build().unwrap();
        })
    })
    .bench_function("container_child_with_hierarchy", |b| {
        let app_container = Container::new(
            RegistriesBuilder::new()
                .provide(|| Ok(()), Runtime)
                .provide(|| Ok(((), ())), App)
                .provide(|| Ok(((), (), ())), Session)
                .provide(|| Ok(((), (), (), ())), Request)
                .provide(|| Ok(((), (), (), (), ())), Action)
                .provide(|| Ok(((), (), (), (), (), ())), Step),
        );
        b.iter(|| {
            let request_container = app_container.clone().enter_build().unwrap();
            let action_container = request_container.enter_build().unwrap();
            let _ = action_container.enter_build().unwrap();
        })
    })
    .bench_function("container_get_with_cache", |b| {
        struct A(Arc<B>, Arc<C>);
        struct B(i32);
        struct C(Arc<CA>);
        struct CA(Arc<CAA>);
        struct CAA(Arc<CAAA>);
        struct CAAA(Arc<CAAAA>);
        struct CAAAA(Arc<CAAAAA>);
        struct CAAAAA;

        let container = Container::new(
            RegistriesBuilder::new()
                .provide(|| (Ok(CAAAAA)), Request)
                .provide(|Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa)), Request)
                .provide(|Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa)), Request)
                .provide(|Inject(caaa): Inject<CAAA>| Ok(CAA(caaa)), Request)
                .provide(|Inject(caa): Inject<CAA>| Ok(CA(caa)), Request)
                .provide(|Inject(ca): Inject<CA>| Ok(C(ca)), Request)
                .provide(|| Ok(B(2)), Request)
                .provide(|Inject(b): Inject<B>, Inject(c): Inject<C>| Ok(A(b, c)), Request),
        );
        let request_container = container.enter_build().unwrap();
        b.iter(|| request_container.get::<A>().unwrap())
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
            RegistriesBuilder::new()
                .provide(|| (Ok(CAAAAA)), Request)
                .provide(|InjectTransient(caaaaa): InjectTransient<CAAAAA>| Ok(CAAAA(caaaaa)), Request)
                .provide(|InjectTransient(caaaa): InjectTransient<CAAAA>| Ok(CAAA(caaaa)), Request)
                .provide(|InjectTransient(caaa): InjectTransient<CAAA>| Ok(CAA(caaa)), Request)
                .provide(|InjectTransient(caa): InjectTransient<CAA>| Ok(CA(caa)), Request)
                .provide(|InjectTransient(ca): InjectTransient<CA>| Ok(C(ca)), Request)
                .provide(|| Ok(B(2)), Request)
                .provide(
                    |InjectTransient(b): InjectTransient<B>, InjectTransient(c): InjectTransient<C>| Ok(A(b, c)),
                    Request,
                ),
        );
        let request_container = container.enter_build().unwrap();
        b.iter(|| request_container.get::<A>().unwrap())
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
