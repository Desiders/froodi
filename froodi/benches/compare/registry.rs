//! Resolution and lifecycle measurements for typed registrations.
#![allow(clippy::pedantic)]

#[path = "../support/graphs.rs"]
mod graphs;

#[cfg(feature = "thread_safe")]
mod bench {
    use super::graphs;

    use criterion::{criterion_group, BatchSize, Criterion};
    use graphs::{typed, S99, T99};
    use std::{hint::black_box, sync::Arc};

    fn resolve(c: &mut Criterion) {
        let mut group = c.benchmark_group("registry_resolution");

        group.bench_function("get_cached", |b| {
            let container = typed::chain(true);
            container.get::<S99>().unwrap();
            b.iter(|| black_box(container.get::<S99>().unwrap()));
        });

        group.bench_function("first_get_chain_100_retained", |b| {
            // Exclude container/plan destruction from the construction measurement.
            b.iter_batched_ref(
                || typed::chain(true),
                |container| black_box(container.get::<S99>().unwrap()),
                BatchSize::SmallInput,
            );
        });

        group.bench_function("get_transient_chain_100", |b| {
            let container = typed::transient_chain();
            b.iter(|| black_box(container.get_transient::<T99>().unwrap()));
        });

        group.finish();
    }

    const REQUESTS: usize = 32;

    fn lifecycle(c: &mut Criterion) {
        let mut group = c.benchmark_group("registry_lifecycle");
        group.bench_function("startup", |b| {
            b.iter_batched(|| (), |_| typed::chain(false), BatchSize::SmallInput);
        });
        group.bench_function("enter_request", |b| {
            let app = typed::chain(false);
            b.iter_batched(|| (), |_| app.clone().enter_build().unwrap(), BatchSize::SmallInput);
        });
        group.bench_function("first_get", |b| {
            let app = typed::chain(false);
            b.iter_batched_ref(
                || app.clone().enter_build().unwrap(),
                |request| black_box(request.get::<S99>().unwrap()),
                BatchSize::SmallInput,
            );
        });
        group.bench_function("cached_get", |b| {
            let app = typed::chain(false);
            let request = app.enter_build().unwrap();
            request.get::<S99>().unwrap();
            b.iter(|| black_box(request.get::<S99>().unwrap()));
        });
        group.bench_function("transient_get_warm_dependencies", |b| {
            let app = typed::chain(false);
            let request = app.enter_build().unwrap();
            request.get::<S99>().unwrap();
            b.iter(|| black_box(request.get_transient::<S99>().unwrap()));
        });
        group.bench_function("close_drop_request", |b| {
            let app = typed::chain(false);
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
        group.bench_function("close_drop_app", |b| {
            b.iter_batched(
                || typed::chain(false),
                |app| {
                    app.close();
                    drop(app);
                },
                BatchSize::SmallInput,
            );
        });
        group.bench_function("complete_32_requests", |b| {
            b.iter(|| {
                let app = typed::chain(false);
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

    #[cfg(feature = "async")]
    fn async_instantiator(c: &mut Criterion) {
        use froodi::{async_impl::Container, registry, DefaultScope::App, InstantiateErrorKind};

        async fn value() -> Result<u64, InstantiateErrorKind> {
            Ok(black_box(7))
        }

        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let container = Container::new(registry! { provide(App, value) });
        let mut group = c.benchmark_group("async_instantiator");
        group.bench_function("transient", |b| {
            b.to_async(&runtime)
                .iter(|| async { black_box(container.get_transient::<u64>().await.unwrap()) });
        });
        group.finish();
    }

    #[cfg(feature = "async")]
    fn mixed_execution(criterion: &mut Criterion) {
        use froodi::{
            async_impl::Container as AsyncContainer,
            registry,
            DefaultScope::{App, Request},
            InjectTransient, InstantiateErrorKind,
        };

        struct Base(usize);
        struct SyncService(usize);
        struct AsyncService(usize);
        struct Handler(usize);

        fn base() -> Result<Base, InstantiateErrorKind> {
            Ok(Base(black_box(7)))
        }

        fn sync_service(InjectTransient(base): InjectTransient<Base>) -> Result<SyncService, InstantiateErrorKind> {
            Ok(SyncService(base.0 + 1))
        }

        async fn async_service(InjectTransient(service): InjectTransient<SyncService>) -> Result<AsyncService, InstantiateErrorKind> {
            Ok(AsyncService(service.0 + 1))
        }

        async fn handler(InjectTransient(service): InjectTransient<AsyncService>) -> Result<Handler, InstantiateErrorKind> {
            Ok(Handler(service.0 + 1))
        }

        fn mixed_container() -> AsyncContainer {
            let sync = registry! { provide(App, base), provide(Request, sync_service) };
            let mixed = registry! {
                provide(Request, async_service),
                provide(Request, handler),
                extend(sync),
            };
            AsyncContainer::new(mixed)
        }

        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut group = criterion.benchmark_group("mixed_execution");
        let sync = AsyncContainer::new(registry! { provide(App, base), provide(App, sync_service) });
        group.bench_function("sync_transient_in_async_container", |bench| {
            bench
                .to_async(&runtime)
                .iter(|| async { black_box(sync.get_transient::<SyncService>().await.unwrap().0) });
        });
        let request = mixed_container().enter().with_scope(Request).build().unwrap();
        group.bench_function("transient_chain", |bench| {
            bench
                .to_async(&runtime)
                .iter(|| async { black_box(request.get_transient::<Handler>().await.unwrap().0) });
        });
        group.bench_function("request_lifecycle", |bench| {
            bench.to_async(&runtime).iter(|| async {
                let app = mixed_container();
                let request = app.clone().enter().with_scope(Request).build().unwrap();
                black_box(request.get::<Handler>().await.unwrap().0);
                black_box(request.get::<Handler>().await.unwrap().0);
                request.close().await;
                app.close().await;
            });
        });
        group.finish();
    }

    #[cfg(feature = "async")]
    criterion_group!(benches, resolve, lifecycle, async_instantiator, mixed_execution);
    #[cfg(not(feature = "async"))]
    criterion_group!(benches, resolve, lifecycle);
}

#[cfg(feature = "thread_safe")]
criterion::criterion_main!(bench::benches);

#[cfg(not(feature = "thread_safe"))]
fn main() {}
