//! Compare inline registrations with equivalent reusable syntax fragments.
#![allow(dead_code)]

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use froodi::{fragment, registry, Container, DefaultScope::App, Inject, InstantiateErrorKind};
use std::hint::black_box;

#[cfg(feature = "async")]
use froodi::{async_impl::Container as AsyncContainer, DefaultScope::Request};
#[cfg(feature = "async")]
use tokio::runtime::Builder;

struct Settings(usize);

struct Repository(usize);

fn settings() -> Result<Settings, InstantiateErrorKind> {
    Ok(Settings(7))
}

fn repository(Inject(settings): Inject<Settings>) -> Result<Repository, InstantiateErrorKind> {
    Ok(Repository(settings.0 + 1))
}

#[fragment(infrastructure)]
registry! {
    provide(App, settings),
    provide(App, repository),
}

fn inline() -> Container {
    Container::new(registry! {
        provide(App, settings),
        provide(App, repository),
    })
}

fn fragmented() -> Container {
    Container::new(registry! {
        extend_fragment(infrastructure!()),
    })
}

#[cfg(feature = "async")]
struct Database(usize);

#[cfg(feature = "async")]
struct Handler(usize);

#[cfg(feature = "async")]
async fn database(Inject(settings): Inject<Settings>) -> Result<Database, InstantiateErrorKind> {
    Ok(Database(settings.0))
}

#[cfg(feature = "async")]
async fn handler(Inject(database): Inject<Database>, Inject(repository): Inject<Repository>) -> Result<Handler, InstantiateErrorKind> {
    Ok(Handler(database.0 + repository.0))
}

#[cfg(feature = "async")]
#[fragment(async_services)]
registry! {
    provide(App, database),
    provide(App, handler),
}

#[cfg(feature = "async")]
fn mixed_inline() -> AsyncContainer {
    AsyncContainer::new(registry! {
        provide(App, settings),
        provide(App, repository),
        provide(App, database),
        provide(App, handler),
    })
}

#[cfg(feature = "async")]
fn mixed_fragmented() -> AsyncContainer {
    AsyncContainer::new(registry! {
        extend_fragment(infrastructure!(), async_services!()),
    })
}

#[cfg(feature = "async")]
#[fragment(request_graph)]
registry! {
    provide(App, settings),
    provide(Request, repository),
    provide(App, database),
    provide(Request, handler),
}

#[cfg(feature = "async")]
fn request_inline() -> AsyncContainer {
    AsyncContainer::new(registry! {
        provide(App, settings),
        provide(Request, repository),
        provide(App, database),
        provide(Request, handler),
    })
}

#[cfg(feature = "async")]
fn request_fragmented() -> AsyncContainer {
    AsyncContainer::new(registry! {
        extend_fragment(request_graph!()),
    })
}

fn compare(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("fragments");
    for (label, build) in [("inline", inline as fn() -> Container), ("fragment", fragmented)] {
        group.bench_function(BenchmarkId::new("startup", label), |bencher| {
            bencher.iter_batched(|| (), |_| black_box(build()), BatchSize::SmallInput);
        });
        group.bench_function(BenchmarkId::new("first", label), |bencher| {
            // Keep container destruction outside the first-resolution measurement.
            bencher.iter_batched_ref(
                build,
                |container| black_box(container.get::<Repository>().unwrap()),
                BatchSize::SmallInput,
            )
        });
        group.bench_function(BenchmarkId::new("cached", label), |bencher| {
            let container = build();
            container.get::<Repository>().unwrap();
            bencher.iter(|| black_box(container.get::<Repository>().unwrap()));
        });
        group.bench_function(BenchmarkId::new("transient_warm_dependencies", label), |bencher| {
            let container = build();
            container.get::<Settings>().unwrap();
            bencher.iter(|| black_box(container.get_transient::<Repository>().unwrap()));
        });
    }
    #[cfg(feature = "async")]
    {
        let runtime = Builder::new_current_thread().build().unwrap();
        for (label, build) in [("inline", mixed_inline as fn() -> AsyncContainer), ("fragment", mixed_fragmented)] {
            group.bench_function(BenchmarkId::new("mixed_transient_warm_dependencies", label), |bencher| {
                let container = build();
                runtime.block_on(container.get::<Handler>()).unwrap();
                bencher
                    .to_async(&runtime)
                    .iter(|| async { black_box(container.get_transient::<Handler>().await.unwrap()) });
            });
            group.bench_function(BenchmarkId::new("mixed_lifecycle", label), |bencher| {
                bencher.to_async(&runtime).iter(|| async {
                    let container = build();
                    black_box(container.get::<Handler>().await.unwrap());
                    container.close().await;
                })
            });
        }
        for (label, build) in [
            ("inline", request_inline as fn() -> AsyncContainer),
            ("fragment", request_fragmented),
        ] {
            let app = build();
            group.bench_function(BenchmarkId::new("request_lifecycle", label), |bencher| {
                bencher.to_async(&runtime).iter(|| async {
                    let request = app.clone().enter_build().unwrap();
                    black_box(request.get::<Handler>().await.unwrap());
                    request.close().await;
                })
            });
        }
    }
    group.finish();
}

criterion_group!(benches, compare);
criterion_main!(benches);
