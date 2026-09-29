//! Resolution cost of current Froodi against the compile-time engine's static and runtime registries.
//! All engines use the checked-in fixtures in `support/graphs.rs`.
#![allow(clippy::pedantic)]

/// The graphs share their types between engines, so both must use `Arc`.
#[cfg(feature = "thread_safe")]
#[path = "support/graphs.rs"]
mod graphs;

#[cfg(feature = "thread_safe")]
mod bench {
    use super::graphs;

    use criterion::{criterion_group, BatchSize, BenchmarkId, Criterion};
    use graphs::{compile_engine as static_graph, compile_engine::indexed, froodi_engine as froodi, Host, Wide, S99, T99};
    use std::{hint::black_box, sync::Arc};

    macro_rules! each_engine {
        ($group:expr, $name:expr, |$engine:ident| $body:expr) => {{
            {
                use froodi as $engine;
                $group.bench_function(BenchmarkId::new($name, "froodi"), $body);
            }
            {
                use static_graph as $engine;
                $group.bench_function(BenchmarkId::new($name, "static"), $body);
            }
            {
                use indexed as $engine;
                $group.bench_function(BenchmarkId::new($name, "indexed"), $body);
            }
        }};
    }

    fn resolve(c: &mut Criterion) {
        let mut group = c.benchmark_group("compile_time_engine");

        each_engine!(group, "container_new_chain_100", |engine| |b| b
            .iter(|| black_box(engine::chain(true))));

        each_engine!(group, "get_cached", |engine| |b| {
            let container = engine::chain(true);
            container.get::<S99>().unwrap();
            b.iter(|| black_box(container.get::<S99>().unwrap()));
        });

        each_engine!(group, "first_get_chain_100", |engine| |b| {
            b.iter_batched(
                || engine::chain(true),
                |container| black_box(container.get::<S99>().unwrap()),
                BatchSize::SmallInput,
            );
        });

        each_engine!(group, "enter_and_resolve_request_chain_100", |engine| |b| {
            let app = engine::chain(false);
            b.iter(|| {
                let request = app.clone().enter_build().unwrap();
                black_box(request.get::<S99>().unwrap());
            });
        });

        each_engine!(group, "enter_build_scope_transition", |engine| |b| {
            let app = engine::chain(false);
            b.iter(|| black_box(app.clone().enter_build().unwrap()));
        });

        each_engine!(group, "get_transient_chain_100", |engine| |b| {
            let container = engine::transient_chain();
            b.iter(|| black_box(container.get_transient::<T99>().unwrap()));
        });

        each_engine!(group, "first_get_wide_16", |engine| |b| {
            b.iter_batched(
                engine::wide,
                |container| black_box(container.get::<Wide>().unwrap()),
                BatchSize::SmallInput,
            );
        });

        each_engine!(group, "enter_and_resolve_captured_closure", |engine| |b| {
            let app = engine::captured();
            b.iter(|| {
                let request = app.clone().enter_build().unwrap();
                black_box(request.get::<Arc<String>>().unwrap());
            });
        });

        group.bench_function(BenchmarkId::new("first_get_mixed_boundary", "static"), |b| {
            b.iter_batched(
                static_graph::mixed,
                |container| black_box(container.get::<Host>().unwrap()),
                BatchSize::SmallInput,
            );
        });

        group.finish();
    }

    criterion_group!(benches, resolve);
}

#[cfg(feature = "thread_safe")]
criterion::criterion_main!(bench::benches);

#[cfg(not(feature = "thread_safe"))]
fn main() {}
