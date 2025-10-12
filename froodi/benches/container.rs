#![allow(dead_code)]

use criterion::{criterion_group, criterion_main, Criterion};
use froodi::{utils::thread_safety::RcThreadSafety, Container, DefaultScope::*, Inject, InjectTransient, RegistryBuilder};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("container_new_with_registry_builder", |b| {
        b.iter(|| {
            Container::new(
                RegistryBuilder::new()
                    .provide(|| Ok(()), Runtime)
                    .provide(|| Ok(((), ())), App)
                    .provide(|| Ok(((), (), ())), Session)
                    .provide(|| Ok(((), (), (), ())), Request)
                    .provide(|| Ok(((), (), (), (), ())), Action)
                    .provide(|| Ok(((), (), (), (), (), ())), Step)
                    .add_finalizer(|_: RcThreadSafety<()>| {})
                    .add_finalizer(|_: RcThreadSafety<((), ())>| {})
                    .add_finalizer(|_: RcThreadSafety<((), (), ())>| {})
                    .add_finalizer(|_: RcThreadSafety<((), (), (), ())>| {})
                    .add_finalizer(|_: RcThreadSafety<((), (), (), (), ())>| {})
                    .add_finalizer(|_: RcThreadSafety<((), (), (), (), (), ())>| {}),
            )
        });
    })
    .bench_function("container_child_with_scope", |b| {
        let runtime_container = Container::new_with_start_scope(
            RegistryBuilder::new()
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
        });
    })
    .bench_function("container_child_with_hierarchy", |b| {
        let app_container = Container::new(
            RegistryBuilder::new()
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
        b.iter(|| request_container.get::<A>().unwrap());
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
                .provide(|| (Ok(CAAAAA)), Runtime)
                .provide(|Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa)), App)
                .provide(|Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa)), Session)
                .provide(|Inject(caaa): Inject<CAAA>| Ok(CAA(caaa)), Request)
                .provide(|Inject(caa): Inject<CAA>| Ok(CA(caa)), Request)
                .provide(|Inject(ca): Inject<CA>| Ok(C(ca)), Action)
                .provide(|| Ok(B(2)), Action)
                .provide(|Inject(b): Inject<B>, Inject(c): Inject<C>| Ok(A(b, c)), Step),
        );
        let scope_container = container.enter().with_scope(Step).build().unwrap();
        b.iter(|| scope_container.get::<A>().unwrap());
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
        b.iter(|| request_container.get::<A>().unwrap());
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
                .provide(|| (Ok(CAAAAA)), Runtime)
                .provide(|InjectTransient(caaaaa): InjectTransient<CAAAAA>| Ok(CAAAA(caaaaa)), App)
                .provide(|InjectTransient(caaaa): InjectTransient<CAAAA>| Ok(CAAA(caaaa)), Session)
                .provide(|InjectTransient(caaa): InjectTransient<CAAA>| Ok(CAA(caaa)), Request)
                .provide(|InjectTransient(caa): InjectTransient<CAA>| Ok(CA(caa)), Request)
                .provide(|InjectTransient(ca): InjectTransient<CA>| Ok(C(ca)), Action)
                .provide(|| Ok(B(2)), Action)
                .provide(
                    |InjectTransient(b): InjectTransient<B>, InjectTransient(c): InjectTransient<C>| Ok(A(b, c)),
                    Step,
                ),
        );
        let scope_container = container.enter().with_scope(Step).build().unwrap();
        b.iter(|| scope_container.get::<A>().unwrap());
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
