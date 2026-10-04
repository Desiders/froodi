#![allow(dead_code)]

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use froodi::{
    registry, Config, Container,
    DefaultScope::{App, Request},
    Inject, InjectTransient, InstantiateErrorKind,
};
use std::{hint::black_box, sync::Arc};

#[derive(Clone)]
struct Root(u32);

struct RequestId(u32);

#[derive(froodi::Construct)]
struct Service {
    root: Arc<Root>,
    #[di(inject_transient)]
    request_id: RequestId,
}

fn manual_service(
    Inject(root): Inject<Root>,
    InjectTransient(request_id): InjectTransient<RequestId>,
) -> Result<Service, InstantiateErrorKind> {
    Ok(Service { root, request_id })
}

fn finish_service(service: Arc<Service>) {
    black_box(service.request_id.0);
}

#[derive(froodi::Construct)]
struct TransientOnly(#[di(inject_transient)] RequestId);

fn manual_transient(InjectTransient(request_id): InjectTransient<RequestId>) -> Result<TransientOnly, InstantiateErrorKind> {
    Ok(TransientOnly(request_id))
}

#[derive(froodi::Construct)]
struct Level1(Arc<Root>);
#[derive(froodi::Construct)]
struct Level2(Arc<Level1>);
#[derive(froodi::Construct)]
struct Level3(Arc<Level2>);
#[derive(froodi::Construct)]
struct Level4(Arc<Level3>);

fn manual_level1(Inject(value): Inject<Root>) -> Result<Level1, InstantiateErrorKind> {
    Ok(Level1(value))
}
fn manual_level2(Inject(value): Inject<Level1>) -> Result<Level2, InstantiateErrorKind> {
    Ok(Level2(value))
}
fn manual_level3(Inject(value): Inject<Level2>) -> Result<Level3, InstantiateErrorKind> {
    Ok(Level3(value))
}
fn manual_level4(Inject(value): Inject<Level3>) -> Result<Level4, InstantiateErrorKind> {
    Ok(Level4(value))
}

fn service_container(derived: bool) -> Container {
    if derived {
        Container::new(registry! {
            provide(App, froodi::instance(Root(7))),
            scope(Request) [
                provide(|| Ok::<_, InstantiateErrorKind>(RequestId(11))),
                construct::<Service>(finalizer = finish_service),
                construct::<TransientOnly>(),
            ],
        })
    } else {
        Container::new(registry! {
            provide(App, froodi::instance(Root(7))),
            scope(Request) [
                provide(|| Ok::<_, InstantiateErrorKind>(RequestId(11))),
                provide(manual_service, finalizer = finish_service),
                provide(manual_transient),
            ],
        })
    }
}

fn deep_container(derived: bool) -> Container {
    if derived {
        Container::new(registry! {
            provide(App, froodi::instance(Root(7))),
            scope(Request) [
                construct::<Level1>(config = Config { cache_provides: false }),
                construct::<Level2>(config = Config { cache_provides: false }),
                construct::<Level3>(config = Config { cache_provides: false }),
                construct::<Level4>(config = Config { cache_provides: false }),
            ],
        })
    } else {
        Container::new(registry! {
            provide(App, froodi::instance(Root(7))),
            scope(Request) [
                provide(manual_level1, config = Config { cache_provides: false }),
                provide(manual_level2, config = Config { cache_provides: false }),
                provide(manual_level3, config = Config { cache_provides: false }),
                provide(manual_level4, config = Config { cache_provides: false }),
            ],
        })
    }
}

fn compare(c: &mut Criterion) {
    for (name, derived) in [("provide", false), ("construct", true)] {
        let mut group = c.benchmark_group(format!("construct_compare/{name}"));
        group.bench_function("first_construction", |b| {
            let app = service_container(derived);
            b.iter_batched_ref(
                || app.clone().enter().with_scope(Request).build().unwrap(),
                |request| black_box(request.get::<Service>().unwrap()),
                BatchSize::SmallInput,
            );
        });
        group.bench_function("cached_get", |b| {
            let request = service_container(derived).enter().with_scope(Request).build().unwrap();
            request.get::<Service>().unwrap();
            b.iter(|| black_box(request.get::<Service>().unwrap()));
        });
        group.bench_function("transient_root", |b| {
            let request = service_container(derived).enter().with_scope(Request).build().unwrap();
            b.iter(|| black_box(request.get_transient::<Service>().unwrap()));
        });
        group.bench_function("transient_field_resolution", |b| {
            let request = service_container(derived).enter().with_scope(Request).build().unwrap();
            b.iter(|| black_box(request.get_transient::<TransientOnly>().unwrap()));
        });
        group.bench_function("deep_chain", |b| {
            let request = deep_container(derived).enter().with_scope(Request).build().unwrap();
            b.iter(|| black_box(request.get_transient::<Level4>().unwrap()));
        });
        group.bench_function("request_lifecycle", |b| {
            let app = service_container(derived);
            b.iter(|| {
                let request = app.clone().enter().with_scope(Request).build().unwrap();
                black_box(request.get::<Service>().unwrap());
                request.close();
            });
        });
        group.finish();
    }
}

criterion_group!(benches, compare);
criterion_main!(benches);
