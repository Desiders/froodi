#![allow(dead_code)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use froodi::{async_impl::Container, async_registry, DefaultScope::*, Inject, InjectTransient};
use std::{
    future::Future,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::runtime::Builder;

const THREADS: usize = 10;
const SCALING_THREADS: [usize; 4] = [1, 2, 4, 8];

async fn run_async_bench_threads<W, F, Fut>(threads: usize, mut make_test_fn: W, iters: u64) -> Duration
where
    W: FnMut() -> F,
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send,
{
    use tokio::sync::Barrier;

    let barrier = Arc::new(Barrier::new(threads));
    let elapsed_handles = Arc::new((0..threads).map(|_| AtomicU64::default()).collect::<Box<[_]>>());

    for i in 0..threads {
        let barrier = barrier.clone();
        let elapsed_handles = elapsed_handles.clone();
        let mut test_fn = make_test_fn();

        tokio::spawn(async move {
            barrier.wait().await;
            let start = Instant::now();
            for _ in 0..iters {
                test_fn().await;
            }
            elapsed_handles[i].store(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
        });
    }

    let mut nanos = Vec::with_capacity(threads);
    for elapsed_handle in elapsed_handles.iter() {
        nanos.push(elapsed_handle.load(Ordering::Relaxed));
    }
    Duration::from_nanos(nanos.iter().sum::<u64>() / nanos.len() as u64)
}

fn criterion_benchmark(c: &mut Criterion) {
    let rt = Builder::new_multi_thread().worker_threads(THREADS).enable_all().build().unwrap();

    let mut group = c.benchmark_group("async_concurrent");
    group.sample_size(100);
    group.warm_up_time(Duration::from_secs(3));

    group.bench_function(BenchmarkId::new("get_single", THREADS), |b| {
        struct A;

        let container = Container::new(async_registry! {
            scope(App) [ provide(async || Ok(A)) ]
        });

        b.to_async(&rt).iter_custom(|iters| {
            let container = container.clone();
            async move {
                run_async_bench_threads(
                    THREADS,
                    || {
                        let container = container.clone();
                        move || {
                            let container = container.clone();
                            async move {
                                container.get::<A>().await.unwrap();
                            }
                        }
                    },
                    (iters + THREADS as u64 - 1) / THREADS as u64,
                )
                .await
            }
        });
    });

    group.bench_function(BenchmarkId::new("get_many", THREADS), |b| {
        struct A(Arc<B>, Arc<C>);
        struct B(i32);
        struct C(Arc<CA>);
        struct CA(Arc<CAA>);
        struct CAA(Arc<CAAA>);
        struct CAAA(Arc<CAAAA>);
        struct CAAAA(Arc<CAAAAA>);
        struct CAAAAA;

        let container = Container::new(async_registry! {
            scope(Runtime) [ provide(async || Ok(CAAAAA)) ],
            scope(App) [ provide(async |Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa))) ],
            scope(Session) [ provide(async |Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa))) ],
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
        })
        .enter()
        .with_scope(Step)
        .build()
        .unwrap();

        b.to_async(&rt).iter_custom(|iters| {
            let container = container.clone();
            async move {
                run_async_bench_threads(
                    THREADS,
                    || {
                        let container = container.clone();
                        move || {
                            let container = container.clone();
                            async move {
                                container.get::<A>().await.unwrap();
                            }
                        }
                    },
                    (iters + THREADS as u64 - 1) / THREADS as u64,
                )
                .await
            }
        });
    });

    group.bench_function(BenchmarkId::new("get_transient_single", THREADS), |b| {
        struct A;

        let container = Container::new(async_registry! {
            scope(App) [ provide(async || Ok(A)) ]
        });

        b.to_async(&rt).iter_custom(|iters| {
            let container = container.clone();
            async move {
                run_async_bench_threads(
                    THREADS,
                    || {
                        let container = container.clone();
                        move || {
                            let container = container.clone();
                            async move {
                                container.get_transient::<A>().await.unwrap();
                            }
                        }
                    },
                    (iters + THREADS as u64 - 1) / THREADS as u64,
                )
                .await
            }
        });
    });

    group.bench_function(BenchmarkId::new("get_transient_many", THREADS), |b| {
        struct A(B, C);
        struct B(i32);
        struct C(CA);
        struct CA(CAA);
        struct CAA(CAAA);
        struct CAAA(CAAAA);
        struct CAAAA(CAAAAA);
        struct CAAAAA;

        let container = Container::new(async_registry! {
            scope(Runtime) [ provide(async || Ok(CAAAAA)) ],
            scope(App) [ provide(async |InjectTransient(caaaaa): InjectTransient<CAAAAA>| Ok(CAAAA(caaaaa))) ],
            scope(Session) [ provide(async |InjectTransient(caaaa): InjectTransient<CAAAA>| Ok(CAAA(caaaa))) ],
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
        })
        .enter()
        .with_scope(Step)
        .build()
        .unwrap();

        b.to_async(&rt).iter_custom(|iters| {
            let container = container.clone();
            async move {
                run_async_bench_threads(
                    THREADS,
                    || {
                        let container = container.clone();
                        move || {
                            let container = container.clone();
                            async move {
                                container.get_transient::<A>().await.unwrap();
                            }
                        }
                    },
                    (iters + THREADS as u64 - 1) / THREADS as u64,
                )
                .await
            }
        });
    });

    for thread_count in SCALING_THREADS {
        group.bench_with_input(BenchmarkId::new("scaling", thread_count), &thread_count, |b, thread_count| {
            struct A(Arc<B>);
            struct B(Arc<C>);
            struct C;

            let container = Container::new(async_registry! {
                scope(App) [ provide(async || Ok(C)) ],
                scope(Request) [
                    provide(async |Inject(c): Inject<C>| Ok(B(c))),
                    provide(async |Inject(b): Inject<B>| Ok(A(b))),
                ],
            })
            .enter()
            .with_scope(Request)
            .build()
            .unwrap();

            b.to_async(&rt).iter_custom(|iters| {
                let container = container.clone();
                async move {
                    run_async_bench_threads(
                        *thread_count,
                        || {
                            let container = container.clone();
                            move || {
                                let container = container.clone();
                                async move {
                                    container.get::<A>().await.unwrap();
                                }
                            }
                        },
                        (iters + *thread_count as u64 - 1) / *thread_count as u64,
                    )
                    .await
                }
            });
        });
    }

    group.finish();
}

criterion_group!(async_concurrent_benches, criterion_benchmark);
criterion_main!(async_concurrent_benches);
