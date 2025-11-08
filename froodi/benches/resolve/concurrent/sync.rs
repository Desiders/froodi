#![allow(dead_code)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use froodi::{registry, Container, DefaultScope::*, Inject, InjectTransient};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Barrier,
    },
    thread,
    time::{Duration, Instant},
};

const THREADS: usize = 10;
const SCALING_THREADS: [usize; 5] = [1, 2, 4, 8, 12];

fn run_bench_threads<W, F>(threads: usize, mut make_test_fn: W, iters: u64) -> Duration
where
    W: FnMut() -> F,
    F: FnMut() + Send + 'static,
{
    let barrier = Arc::new(Barrier::new(threads + 1));
    let elapsed_handles = Arc::new((0..threads).map(|_| AtomicU64::default()).collect::<Box<[_]>>());

    thread::scope(|s| {
        for i in 0..threads {
            let barrier = barrier.clone();
            let elapsed_handles = elapsed_handles.clone();
            let mut test_fn = make_test_fn();

            s.spawn(move || {
                barrier.wait();
                let start = Instant::now();
                for _ in 0..iters {
                    test_fn();
                }
                elapsed_handles[i].store(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
            });
        }

        barrier.wait();
    });

    let mut nanos = Vec::with_capacity(threads);
    for elapsed_handle in elapsed_handles.iter() {
        nanos.push(elapsed_handle.load(Ordering::Relaxed));
    }
    Duration::from_nanos(nanos.iter().sum::<u64>() / nanos.len() as u64)
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent");
    group.sample_size(30);
    group.warm_up_time(Duration::from_secs(3));

    group.bench_function(BenchmarkId::new("get_single", THREADS), |b| {
        struct A;

        let container = Container::new(registry! {
            scope(App) [ provide(|| Ok(A)) ]
        });

        b.iter_custom(|iters| {
            run_bench_threads(
                THREADS,
                || {
                    let container = container.clone();
                    move || {
                        container.get::<A>().unwrap();
                    }
                },
                (iters + THREADS as u64 - 1) / THREADS as u64,
            )
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

        let container = Container::new(registry! {
            scope(Runtime) [
                provide(|| Ok(CAAAAA)),
            ],
            scope(App) [
                provide(|Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa))),
            ],
            scope(Session) [
                provide(|Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa))),
            ],
            scope(Request) [
                provide(|Inject(caaa): Inject<CAAA>| Ok(CAA(caaa))),
                provide(|Inject(caa): Inject<CAA>| Ok(CA(caa))),
            ],
            scope(Action) [
                provide(|Inject(ca): Inject<CA>| Ok(C(ca))),
                provide(|| Ok(B(2))),
            ],
            scope(Step) [
                provide(|Inject(b): Inject<B>, Inject(c): Inject<C>| Ok(A(b, c))),
            ],
        })
        .enter()
        .with_scope(Step)
        .build()
        .unwrap();

        b.iter_custom(|iters| {
            run_bench_threads(
                THREADS,
                || {
                    let container = container.clone();
                    move || {
                        container.get::<A>().unwrap();
                    }
                },
                (iters + THREADS as u64 - 1) / THREADS as u64,
            )
        });
    });

    group.bench_function(BenchmarkId::new("get_transient_single", THREADS), |b| {
        struct A;

        let container = Container::new(registry! {
            scope(App) [ provide(|| Ok(A)) ]
        });

        b.iter_custom(|iters| {
            run_bench_threads(
                THREADS,
                || {
                    let container = container.clone();
                    move || {
                        container.get_transient::<A>().unwrap();
                    }
                },
                (iters + THREADS as u64 - 1) / THREADS as u64,
            )
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

        let container = Container::new(registry! {
            scope(Runtime) [
                provide(|| Ok(CAAAAA)),
            ],
            scope(App) [
                provide(|InjectTransient(caaaaa): InjectTransient<CAAAAA>| Ok(CAAAA(caaaaa))),
            ],
            scope(Session) [
                provide(|InjectTransient(caaaa): InjectTransient<CAAAA>| Ok(CAAA(caaaa))),
            ],
            scope(Request) [
                provide(|InjectTransient(caaa): InjectTransient<CAAA>| Ok(CAA(caaa))),
                provide(|InjectTransient(caa): InjectTransient<CAA>| Ok(CA(caa))),
            ],
            scope(Action) [
                provide(|InjectTransient(ca): InjectTransient<CA>| Ok(C(ca))),
                provide(|| Ok(B(2))),
            ],
            scope(Step) [
                provide(|InjectTransient(b): InjectTransient<B>, InjectTransient(c): InjectTransient<C>| Ok(A(b, c))),
            ],
        })
        .enter()
        .with_scope(Step)
        .build()
        .unwrap();

        b.iter_custom(|iters| {
            run_bench_threads(
                THREADS,
                || {
                    let container = container.clone();
                    move || {
                        container.get_transient::<A>().unwrap();
                    }
                },
                (iters + THREADS as u64 - 1) / THREADS as u64,
            )
        });
    });

    for thread_count in SCALING_THREADS {
        group.bench_with_input(BenchmarkId::new("scaling", thread_count), &thread_count, |b, thread_count| {
            struct A(Arc<B>);
            struct B(Arc<C>);
            struct C;

            let container = Container::new(registry! {
                scope(App) [
                    provide(|| Ok(C)),
                ],
                scope(Request) [
                    provide(|Inject(c): Inject<C>| Ok(B(c))),
                    provide(|Inject(b): Inject<B>| Ok(A(b))),
                ],
            })
            .enter()
            .with_scope(Request)
            .build()
            .unwrap();

            b.iter_custom(|iters| {
                run_bench_threads(
                    *thread_count,
                    || {
                        let container = container.clone();
                        move || {
                            container.get::<A>().unwrap();
                        }
                    },
                    (iters + *thread_count as u64 - 1) / *thread_count as u64,
                )
            });
        });
    }

    group.finish();
}

criterion_group!(concurrent_benches, criterion_benchmark);
criterion_main!(concurrent_benches);
