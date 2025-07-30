#![allow(dead_code)]

use criterion::{criterion_group, criterion_main, Criterion};
use froodi::{Container, DefaultScope::*, Inject, RegistriesBuilder};
use std::sync::Arc;

struct A(Arc<B>, Arc<C>);
struct B(i32);
struct C(Arc<CA>);
struct CA(Arc<CAA>);
struct CAA(Arc<CAAA>);
struct CAAA(Arc<CAAAA>);
struct CAAAA(Arc<CAAAAA>);
struct CAAAAA;

#[inline]
fn container_new_with_registries_builder() -> Container {
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
}

#[inline]
fn container_child_with_scope(runtime_container: Container) {
    let app_container = runtime_container.enter().with_scope(App).build().unwrap();
    let session_container = app_container.enter().with_scope(Session).build().unwrap();
    let request_container = session_container.enter().with_scope(Request).build().unwrap();
    let action_container = request_container.enter().with_scope(Action).build().unwrap();
    let _ = action_container.enter().with_scope(Step).build().unwrap();
}

#[inline]
fn container_child_with_hierarchy(runtime_container: Container) {
    let app_container = runtime_container.enter().with_scope(App).build().unwrap();
    let session_container = app_container.enter().with_scope(Session).build().unwrap();
    let request_container = session_container.enter().with_scope(Request).build().unwrap();
    let action_container = request_container.enter().with_scope(Action).build().unwrap();
    let _ = action_container.enter().with_scope(Step).build().unwrap();
}

#[inline]
fn container_get(container: &Container) {
    let _ = container.get::<A>().unwrap();
}

#[inline]
fn container_close(container: &Container) {
    let _ = container.get::<A>().unwrap();

    container.close();
}

fn criterion_benchmark(c: &mut Criterion) {
    let container_1 = Container::new(
        RegistriesBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(((), ())), App)
            .provide(|| Ok(((), (), ())), Session)
            .provide(|| Ok(((), (), (), ())), Request)
            .provide(|| Ok(((), (), (), (), ())), Action)
            .provide(|| Ok(((), (), (), (), (), ())), Step),
    );
    let container_2 = Container::new(
        RegistriesBuilder::new()
            .provide(|| (Ok(CAAAAA)), Request)
            .provide(|Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa)), Request)
            .provide(|Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa)), Request)
            .provide(|Inject(caaa): Inject<CAAA>| Ok(CAA(caaa)), Request)
            .provide(|Inject(caa): Inject<CAA>| Ok(CA(caa)), Request)
            .provide(|Inject(ca): Inject<CA>| Ok(C(ca)), Request)
            .provide(|| Ok(B(2)), Request)
            .provide(|Inject(b): Inject<B>, Inject(c): Inject<C>| Ok(A(b, c)), Request)
            .add_finalizer(|_: Arc<CAAAAA>| {})
            .add_finalizer(|_: Arc<CAAAA>| {})
            .add_finalizer(|_: Arc<CAAA>| {})
            .add_finalizer(|_: Arc<CAA>| {})
            .add_finalizer(|_: Arc<CA>| {})
            .add_finalizer(|_: Arc<C>| {})
            .add_finalizer(|_: Arc<B>| {})
            .add_finalizer(|_: Arc<A>| {}),
    );
    let container_3 = Container::new(
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

    c.bench_function("container_new_with_registries_builder", |b| {
        b.iter(|| container_new_with_registries_builder())
    })
    .bench_function("container_child_with_scope", |b| {
        b.iter(|| container_child_with_scope(container_1.clone()))
    })
    .bench_function("container_child_with_hierarchy", |b| {
        b.iter(|| container_child_with_hierarchy(container_1.clone()))
    })
    .bench_function("container_get", |b| b.iter(|| container_get(&mut container_2.clone())))
    .bench_function("container_get_with_cache", |b| {
        let mut container_2 = container_2.clone();
        b.iter(|| container_get(&mut container_2))
    })
    .bench_function("container_close", |b| b.iter(|| container_close(&mut container_2.clone())))
    .bench_function("container_close_without_finalizers", |b| {
        b.iter(|| container_close(&mut container_3.clone()))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
