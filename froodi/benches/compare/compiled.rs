//! Dynamic and compiled registrations use the same Froodi lifecycle.
//! Both frontends use the fixtures in `support/graphs.rs`.
#![allow(clippy::pedantic)]

#[path = "../support/graphs.rs"]
mod graphs;

#[cfg(feature = "thread_safe")]
mod bench {
    use super::graphs;

    use criterion::{criterion_group, BatchSize, BenchmarkId, Criterion};
    use graphs::{compiled, dynamic, S99, T99};
    use std::{hint::black_box, sync::Arc};

    macro_rules! each_engine {
        ($group:expr, $name:expr, |$engine:ident| $body:expr) => {{
            {
                use dynamic as $engine;
                $group.bench_function(BenchmarkId::new($name, "dynamic"), $body);
            }
            {
                use compiled as $engine;
                $group.bench_function(BenchmarkId::new($name, "compiled"), $body);
            }
        }};
    }

    fn resolve(c: &mut Criterion) {
        let mut group = c.benchmark_group("registry_resolution");

        each_engine!(group, "get_cached", |engine| |b| {
            let container = engine::chain(true);
            container.get::<S99>().unwrap();
            b.iter(|| black_box(container.get::<S99>().unwrap()));
        });

        each_engine!(group, "first_get_chain_100_retained", |engine| |b| {
            // Exclude container/plan destruction from the construction measurement.
            b.iter_batched_ref(
                || engine::chain(true),
                |container| black_box(container.get::<S99>().unwrap()),
                BatchSize::SmallInput,
            );
        });

        each_engine!(group, "get_transient_chain_100", |engine| |b| {
            let container = engine::transient_chain();
            b.iter(|| black_box(container.get_transient::<T99>().unwrap()));
        });

        group.finish();
    }

    const REQUESTS: usize = 32;

    fn lifecycle(c: &mut Criterion) {
        let mut group = c.benchmark_group("native_lifecycle");
        each_engine!(group, "startup", |engine| |b| {
            b.iter_batched(|| (), |_| engine::chain(false), BatchSize::SmallInput);
        });
        each_engine!(group, "enter_request", |engine| |b| {
            let app = engine::chain(false);
            b.iter_batched(|| (), |_| app.clone().enter_build().unwrap(), BatchSize::SmallInput);
        });
        each_engine!(group, "first_get", |engine| |b| {
            let app = engine::chain(false);
            b.iter_batched_ref(
                || app.clone().enter_build().unwrap(),
                |request| black_box(request.get::<S99>().unwrap()),
                BatchSize::SmallInput,
            );
        });
        each_engine!(group, "cached_get", |engine| |b| {
            let app = engine::chain(false);
            let request = app.enter_build().unwrap();
            request.get::<S99>().unwrap();
            b.iter(|| black_box(request.get::<S99>().unwrap()));
        });
        each_engine!(group, "transient_get_warm_dependencies", |engine| |b| {
            let app = engine::chain(false);
            let request = app.enter_build().unwrap();
            request.get::<S99>().unwrap();
            b.iter(|| black_box(request.get_transient::<S99>().unwrap()));
        });
        each_engine!(group, "close_drop_request", |engine| |b| {
            let app = engine::chain(false);
            b.iter_batched(
                || {
                    let request = app.clone().enter_build().unwrap();
                    request.get::<S99>().unwrap();
                    request
                },
                |request| {
                    request.close();
                    drop(request);
                },
                BatchSize::SmallInput,
            );
        });
        each_engine!(group, "close_drop_app", |engine| |b| {
            b.iter_batched(
                || engine::chain(false),
                |app| {
                    app.close();
                    drop(app);
                },
                BatchSize::SmallInput,
            );
        });
        each_engine!(group, "complete_32_requests", |engine| |b| {
            b.iter(|| {
                let app = engine::chain(false);
                for _ in 0..REQUESTS {
                    let request = app.clone().enter_build().unwrap();
                    let first = request.get::<S99>().unwrap();
                    let cached = request.get::<S99>().unwrap();
                    assert!(Arc::ptr_eq(&first, &cached));
                    black_box(request.get_transient::<S99>().unwrap());
                    drop((first, cached));
                    request.close();
                    drop(request);
                }
                app.close();
                drop(app);
            });
        });
        group.finish();
    }

    criterion_group!(benches, resolve, lifecycle);
}

#[cfg(feature = "thread_safe")]
criterion::criterion_main!(bench::benches);

#[cfg(not(feature = "thread_safe"))]
fn main() {}
