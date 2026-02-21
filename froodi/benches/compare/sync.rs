#![allow(dead_code)]

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use froodi::{registry, utils::thread_safety::RcThreadSafety, Container, DefaultScope::*, Inject, InjectTransient};
use std::hint::black_box;

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("compare_container_resolve");
    group.throughput(Throughput::Elements(1));

    group.bench_function("compare_enter_close_scope_no_resolve", |b| {
        let container = Container::new(registry! {
            scope(App) [
                provide(|| Ok(42_i32)),
            ],
        });
        assert_eq!(*container.get::<i32>().unwrap(), 42);

        b.iter(|| {
            let _ = container.clone().enter().with_scope(Request).build().unwrap();
        });
    });

    group.bench_function("compare_enter_close_scope_resolve_once", |b| {
        let container = Container::new(registry! {
            scope(App) [
                provide(|| Ok(42_i32)),
            ],
        });
        let scope = container.clone().enter().with_scope(Request).build().unwrap();
        assert_eq!(*scope.get::<i32>().unwrap(), 42);

        b.iter(|| {
            let scope = container.clone().enter().with_scope(Request).build().unwrap();
            black_box(scope.get::<i32>().unwrap());
        });
    });

    group.bench_function("compare_enter_close_scope_resolve_100_instance", |b| {
        let container = Container::new(registry! {
            scope(App) [
                provide(|| Ok(42_i32)),
            ],
        });
        let scope = container.clone().enter().with_scope(Request).build().unwrap();
        let values = [
            scope.get_transient::<i32>().unwrap(),
            scope.get_transient::<i32>().unwrap(),
            scope.get_transient::<i32>().unwrap(),
        ];
        assert!(values.iter().all(|&v| v == 42));

        b.iter(|| {
            let scope = container.clone().enter().with_scope(Request).build().unwrap();
            for _ in 0..100 {
                black_box(scope.get::<i32>().unwrap());
            }
        });
    });

    group.bench_function("compare_enter_close_scope_resolve_scoped_100", |b| {
        struct ScopedService;

        let container = Container::new(registry! {
            scope(Request) [
                provide(|| Ok(ScopedService)),
            ],
        });

        let first_scope = container.clone().enter().with_scope(Request).build().unwrap();
        let first = first_scope.get::<ScopedService>().unwrap();
        let second = first_scope.get::<ScopedService>().unwrap();
        assert!(RcThreadSafety::ptr_eq(&first, &second));

        let second_scope = container.clone().enter().with_scope(Request).build().unwrap();
        let third = second_scope.get::<ScopedService>().unwrap();
        assert!(!RcThreadSafety::ptr_eq(&first, &third));

        b.iter(|| {
            let scope = container.clone().enter().with_scope(Request).build().unwrap();
            for _ in 0..100 {
                black_box(scope.get::<ScopedService>().unwrap());
            }
        });
    });

    group.bench_function("compare_resolve_singleton", |b| {
        struct SingletonService;

        let container = Container::new(registry! {
            scope(App) [
                provide(|| Ok(SingletonService)),
            ],
        });

        let first = container.get::<SingletonService>().unwrap();
        let second = container.get::<SingletonService>().unwrap();
        assert!(RcThreadSafety::ptr_eq(&first, &second));

        b.iter(|| black_box(container.get::<SingletonService>().unwrap()));
    });

    group.bench_function("compare_resolve_transient", |b| {
        struct TransientService;

        let container = Container::new(registry! {
            scope(App) [
                provide(|| Ok(TransientService)),
            ],
        });

        b.iter(|| black_box(container.get_transient::<TransientService>().unwrap()));
    });

    group.bench_function("compare_resolve_deep_transient_chain", |b| {
        struct Dep0;
        struct Dep1(Dep0);
        struct Dep2(Dep1);
        struct Dep3(Dep2);
        struct Dep4(Dep3);
        struct Root(Dep4);

        let container = Container::new(registry! {
            scope(App) [
                provide(|| Ok(Dep0)),
                provide(|InjectTransient(dep0): InjectTransient<Dep0>| Ok(Dep1(dep0))),
                provide(|InjectTransient(dep1): InjectTransient<Dep1>| Ok(Dep2(dep1))),
                provide(|InjectTransient(dep2): InjectTransient<Dep2>| Ok(Dep3(dep2))),
                provide(|InjectTransient(dep3): InjectTransient<Dep3>| Ok(Dep4(dep3))),
                provide(|InjectTransient(dep4): InjectTransient<Dep4>| Ok(Root(dep4))),
            ],
        });

        b.iter(|| black_box(container.get_transient::<Root>().unwrap()));
    });

    group.bench_function("compare_resolve_wide_transient_graph", |b| {
        struct DepA;
        struct DepB;
        struct DepC;
        struct DepD;
        struct DepE;
        struct Root(DepA, DepB, DepC, DepD, DepE);

        let container = Container::new(registry! {
            scope(App) [
                provide(|| Ok(DepA)),
                provide(|| Ok(DepB)),
                provide(|| Ok(DepC)),
                provide(|| Ok(DepD)),
                provide(|| Ok(DepE)),
                provide(
                    |InjectTransient(dep_a): InjectTransient<DepA>,
                     InjectTransient(dep_b): InjectTransient<DepB>,
                     InjectTransient(dep_c): InjectTransient<DepC>,
                     InjectTransient(dep_d): InjectTransient<DepD>,
                     InjectTransient(dep_e): InjectTransient<DepE>| Ok(Root(dep_a, dep_b, dep_c, dep_d, dep_e)),
                ),
            ],
        });

        b.iter(|| black_box(container.get_transient::<Root>().unwrap()));
    });

    group.bench_function("compare_resolve_mixed_lifetimes", |b| {
        struct SharedDependency;
        struct PerResolveDependency(RcThreadSafety<SharedDependency>);
        struct RootScopedService(PerResolveDependency);

        let container = Container::new(registry! {
            scope(App) [
                provide(|| Ok(SharedDependency)),
            ],
            scope(Request) [
                provide(|Inject(shared): Inject<SharedDependency>| Ok(PerResolveDependency(shared))),
                provide(|InjectTransient(dep): InjectTransient<PerResolveDependency>| Ok(RootScopedService(dep))),
            ],
        });

        let first_scope = container.clone().enter().with_scope(Request).build().unwrap();
        let first = first_scope.get::<RootScopedService>().unwrap();
        let second = first_scope.get::<RootScopedService>().unwrap();
        assert!(RcThreadSafety::ptr_eq(&first, &second));

        let second_scope = container.clone().enter().with_scope(Request).build().unwrap();
        let third = second_scope.get::<RootScopedService>().unwrap();
        assert!(RcThreadSafety::ptr_eq(&first.0 .0, &third.0 .0));

        b.iter(|| {
            let scope = container.clone().enter().with_scope(Request).build().unwrap();
            black_box(scope.get::<RootScopedService>().unwrap());
        });
    });

    group.bench_function("compare_resolve_generated_scoped_grid", |b| {
        struct Top0;
        struct Top1;
        struct Top2;
        struct Top3;
        struct Top4;
        struct Top5;

        struct Layer1(
            RcThreadSafety<Top0>,
            RcThreadSafety<Top1>,
            RcThreadSafety<Top2>,
            RcThreadSafety<Top3>,
            RcThreadSafety<Top4>,
            RcThreadSafety<Top5>,
        );
        struct Layer2(
            RcThreadSafety<Layer1>,
            RcThreadSafety<Layer1>,
            RcThreadSafety<Layer1>,
            RcThreadSafety<Layer1>,
            RcThreadSafety<Layer1>,
            RcThreadSafety<Layer1>,
        );
        struct Layer3(
            RcThreadSafety<Layer2>,
            RcThreadSafety<Layer2>,
            RcThreadSafety<Layer2>,
            RcThreadSafety<Layer2>,
            RcThreadSafety<Layer2>,
            RcThreadSafety<Layer2>,
        );
        struct Layer4(
            RcThreadSafety<Layer3>,
            RcThreadSafety<Layer3>,
            RcThreadSafety<Layer3>,
            RcThreadSafety<Layer3>,
            RcThreadSafety<Layer3>,
            RcThreadSafety<Layer3>,
        );
        struct Layer5(
            RcThreadSafety<Layer4>,
            RcThreadSafety<Layer4>,
            RcThreadSafety<Layer4>,
            RcThreadSafety<Layer4>,
            RcThreadSafety<Layer4>,
            RcThreadSafety<Layer4>,
        );
        struct Layer6(
            RcThreadSafety<Layer5>,
            RcThreadSafety<Layer5>,
            RcThreadSafety<Layer5>,
            RcThreadSafety<Layer5>,
            RcThreadSafety<Layer5>,
            RcThreadSafety<Layer5>,
        );
        struct Bottom(
            RcThreadSafety<Layer6>,
            RcThreadSafety<Layer6>,
            RcThreadSafety<Layer6>,
            RcThreadSafety<Layer6>,
            RcThreadSafety<Layer6>,
            RcThreadSafety<Layer6>,
        );

        let container = Container::new(registry! {
            scope(App) [
                provide(|| Ok(Top0)),
                provide(|| Ok(Top1)),
                provide(|| Ok(Top2)),
                provide(|| Ok(Top3)),
                provide(|| Ok(Top4)),
                provide(|| Ok(Top5)),
            ],
            scope(Request) [
                provide(
                    |Inject(top0): Inject<Top0>,
                     Inject(top1): Inject<Top1>,
                     Inject(top2): Inject<Top2>,
                     Inject(top3): Inject<Top3>,
                     Inject(top4): Inject<Top4>,
                     Inject(top5): Inject<Top5>| Ok(Layer1(top0, top1, top2, top3, top4, top5)),
                ),
                provide(
                    |Inject(layer1a): Inject<Layer1>,
                     Inject(layer1b): Inject<Layer1>,
                     Inject(layer1c): Inject<Layer1>,
                     Inject(layer1d): Inject<Layer1>,
                     Inject(layer1e): Inject<Layer1>,
                     Inject(layer1f): Inject<Layer1>| Ok(Layer2(layer1a, layer1b, layer1c, layer1d, layer1e, layer1f)),
                ),
                provide(
                    |Inject(layer2a): Inject<Layer2>,
                     Inject(layer2b): Inject<Layer2>,
                     Inject(layer2c): Inject<Layer2>,
                     Inject(layer2d): Inject<Layer2>,
                     Inject(layer2e): Inject<Layer2>,
                     Inject(layer2f): Inject<Layer2>| Ok(Layer3(layer2a, layer2b, layer2c, layer2d, layer2e, layer2f)),
                ),
                provide(
                    |Inject(layer3a): Inject<Layer3>,
                     Inject(layer3b): Inject<Layer3>,
                     Inject(layer3c): Inject<Layer3>,
                     Inject(layer3d): Inject<Layer3>,
                     Inject(layer3e): Inject<Layer3>,
                     Inject(layer3f): Inject<Layer3>| Ok(Layer4(layer3a, layer3b, layer3c, layer3d, layer3e, layer3f)),
                ),
                provide(
                    |Inject(layer4a): Inject<Layer4>,
                     Inject(layer4b): Inject<Layer4>,
                     Inject(layer4c): Inject<Layer4>,
                     Inject(layer4d): Inject<Layer4>,
                     Inject(layer4e): Inject<Layer4>,
                     Inject(layer4f): Inject<Layer4>| Ok(Layer5(layer4a, layer4b, layer4c, layer4d, layer4e, layer4f)),
                ),
                provide(
                    |Inject(layer5a): Inject<Layer5>,
                     Inject(layer5b): Inject<Layer5>,
                     Inject(layer5c): Inject<Layer5>,
                     Inject(layer5d): Inject<Layer5>,
                     Inject(layer5e): Inject<Layer5>,
                     Inject(layer5f): Inject<Layer5>| Ok(Layer6(layer5a, layer5b, layer5c, layer5d, layer5e, layer5f)),
                ),
                provide(
                    |Inject(layer6a): Inject<Layer6>,
                     Inject(layer6b): Inject<Layer6>,
                     Inject(layer6c): Inject<Layer6>,
                     Inject(layer6d): Inject<Layer6>,
                     Inject(layer6e): Inject<Layer6>,
                     Inject(layer6f): Inject<Layer6>| Ok(Bottom(layer6a, layer6b, layer6c, layer6d, layer6e, layer6f)),
                ),
            ],
        });

        let first_scope = container.clone().enter().with_scope(Request).build().unwrap();
        let first = first_scope.get::<Bottom>().unwrap();
        let second = first_scope.get::<Bottom>().unwrap();
        assert!(RcThreadSafety::ptr_eq(&first, &second));

        let second_scope = container.clone().enter().with_scope(Request).build().unwrap();
        let third = second_scope.get::<Bottom>().unwrap();
        assert!(!RcThreadSafety::ptr_eq(&first, &third));
        assert!(!RcThreadSafety::ptr_eq(&first.0, &third.0));

        let first_top = first.0 .0 .0 .0 .0 .0 .0.clone();
        let third_top = third.0 .0 .0 .0 .0 .0 .0.clone();
        assert!(RcThreadSafety::ptr_eq(&first_top, &third_top));

        b.iter(|| {
            let scope = container.clone().enter().with_scope(Request).build().unwrap();
            black_box(scope.get::<Bottom>().unwrap());
        });
    });
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
