#[cfg(feature = "unified")]
use compiled_enabler::{
    registry as reexported_registry,
    scopes::{App as StaticApp, Request as StaticRequest},
};
#[cfg(all(feature = "unified", feature = "async"))]
use di::async_impl::{TypedContainer as AsyncTypedContainer, TypedContainerExt as _};
#[cfg(all(feature = "compiled", feature = "async"))]
use di::compiled_async_registry as async_registry;
#[cfg(all(feature = "unified", feature = "async"))]
use di::compiled_async_registry as renamed_async_registry;
#[cfg(any(feature = "compiled", feature = "async", feature = "unified"))]
use di::Inject;
#[cfg(feature = "async")]
use di::{
    async_impl::{Container as AsyncContainer, RegistryWithSync},
    async_registry as dynamic_async_registry,
    DefaultScope::Request,
};
#[cfg(feature = "compiled")]
use di::{compiled_registry as registry, DefaultScope, InjectTransient, RuntimeDependency};
use di::{
    instance, registry as dynamic_registry, Container, DefaultScope::App, DependencyResolver, InstantiateErrorKind, Registry,
    ResolveErrorKind, Scope,
};
#[cfg(feature = "unified")]
use di::{TypedContainer, TypedContainerExt as _};
#[cfg(feature = "compiled")]
use std::panic::catch_unwind;
#[cfg(feature = "async")]
use tokio::runtime::Builder as RuntimeBuilder;

struct Custom(u32);

impl DependencyResolver for Custom {
    type Error = ResolveErrorKind;

    fn resolve(container: &Container) -> Result<Self, Self::Error> {
        container.get::<u32>().map(|value| Self(*value))
    }

    #[cfg(feature = "async")]
    async fn resolve_async(container: &AsyncContainer) -> Result<Self, Self::Error> {
        container.get::<u32>().await.map(|value| Self(*value))
    }
}

fn dynamic(value: Custom) -> Result<String, InstantiateErrorKind> {
    Ok(value.0.to_string())
}

fn scoped<S: Scope>(registry: Registry, scope: S) -> Container {
    Container::new_with_start_scope::<S>(registry, scope)
}

fn check_dynamic() {
    let registry = dynamic_registry! {
        provide(App, instance(1u32)),
        provide(App, instance(7u32)),
        provide(App, dynamic),
    };
    let container = scoped(registry.clone(), App);
    assert_eq!(&*container.get::<String>().unwrap(), "7");
    assert_eq!(*Container::new_with_start_scope(registry, App).get::<u32>().unwrap(), 7);
}

#[cfg(feature = "compiled")]
fn check_compiled() {
    let suffix = String::from("!");
    let registry = registry! {
        provide(App, instance(7u32)),
        provide(App, move |number: Inject<u32>, fresh: InjectTransient<u32>, custom: RuntimeDependency<Custom>| {
            assert_eq!(*number.0, fresh.0);
            assert_eq!(*number.0, custom.0.0);
            Ok(format!("{}{}", number.0, suffix))
        }),
    };
    let container = Container::new_compiled_with_start_scope::<DefaultScope, _>(registry, App);
    assert_eq!(&*container.get::<String>().unwrap(), "7!");

    check_erased();
}

#[cfg(feature = "compiled")]
fn compiled_fragment() -> Registry {
    registry! { provide(App, instance(11u32)) }.into_registry()
}

#[cfg(feature = "compiled")]
struct CycleLeft;
#[cfg(feature = "compiled")]
struct CycleRight;

#[cfg(feature = "compiled")]
fn cyclic_fragment() -> Registry {
    registry! {
        provide(App, |_: Inject<CycleRight>| Ok(CycleLeft)),
        provide(App, |_: Inject<CycleLeft>| Ok(CycleRight)),
    }
    .into_registry()
}

#[cfg(feature = "compiled")]
fn check_erased() {
    let container = Container::new(dynamic_registry! {
        provide(App, |number: Inject<u32>| Ok::<_, InstantiateErrorKind>(*number.0 as u64)),
        extend(compiled_fragment()),
    });
    assert_eq!(*container.get::<u64>().unwrap(), 11);

    let container = Container::new(dynamic_registry! {
        extend(cyclic_fragment(), dynamic_registry! {
            provide(App, || Ok::<_, InstantiateErrorKind>(CycleLeft)),
        }),
    });
    assert!(container.get::<CycleRight>().is_ok());
    assert!(catch_unwind(|| Container::new(cyclic_fragment())).is_err());
}

#[cfg(all(feature = "compiled", feature = "async"))]
fn compiled_async_fragment() -> RegistryWithSync {
    async_registry! {
        provide(App, async || Ok(13u32)),
        provide(App, async |number: Inject<u32>| Ok(number.0.to_string())),
    }
    .into_async_registry()
}

#[cfg(all(feature = "compiled", feature = "async"))]
fn cyclic_async_fragment() -> RegistryWithSync {
    async_registry! {
        provide(App, async |_: Inject<CycleRight>| Ok(CycleLeft)),
        provide(App, async |_: Inject<CycleLeft>| Ok(CycleRight)),
    }
    .into_async_registry()
}

#[cfg(feature = "async")]
fn check_async() {
    fn scoped<S: Scope + Clone>(registry: RegistryWithSync, scope: S) -> AsyncContainer {
        AsyncContainer::new_with_start_scope::<S>(registry, scope)
    }

    RuntimeBuilder::new_current_thread().build().unwrap().block_on(async {
        let dynamic = dynamic_async_registry! {
            provide(App, async || Ok::<_, InstantiateErrorKind>(1u32)),
            provide(App, async || Ok::<_, InstantiateErrorKind>(7u32)),
            provide(App, async |custom: Custom| Ok::<_, InstantiateErrorKind>(custom.0.to_string())),
        };
        assert_eq!(
            *AsyncContainer::new_with_start_scope(dynamic.clone(), App)
                .get::<u32>()
                .await
                .unwrap(),
            7
        );
        assert_eq!(&*scoped(dynamic, App).get::<String>().await.unwrap(), "7");

        struct NativeService;

        let container = AsyncContainer::new(dynamic_async_registry! {
            provide(App, async |_: Inject<u16>| Ok::<_, InstantiateErrorKind>(NativeService)),
            extend(dynamic_registry! { provide(Request, instance(1u16)) }),
        });
        assert!(container.get::<NativeService>().await.is_err());

        #[cfg(feature = "compiled")]
        {
            let container = AsyncContainer::new_compiled_with_start_scope::<DefaultScope, _>(
                async_registry! {
                    provide(App, async || Ok(7u32)),
                    provide(App, async |number: Inject<u32>, custom: RuntimeDependency<Custom>, fresh: InjectTransient<u32>| {
                        assert_eq!(*number.0, custom.0.0);
                        Ok(format!("{}-{}", number.0, fresh.0))
                    }),
                },
                App,
            );
            assert_eq!(&*container.get::<String>().await.unwrap(), "7-7");

            let container = AsyncContainer::new(dynamic_async_registry! {
                provide(App, async |text: Inject<String>| Ok::<_, InstantiateErrorKind>(text.0.len())),
                extend(compiled_async_fragment()),
            });
            assert_eq!(*container.get::<usize>().await.unwrap(), 2);

            let container = AsyncContainer::new(dynamic_async_registry! {
                extend(cyclic_async_fragment(), dynamic_async_registry! {
                    provide(App, async || Ok::<_, InstantiateErrorKind>(CycleLeft)),
                }),
            });
            assert!(container.get::<CycleRight>().await.is_ok());
            assert!(catch_unwind(|| AsyncContainer::new(cyclic_async_fragment())).is_err());
        }
    });
}

fn main() {
    check_dynamic();
    #[cfg(feature = "compiled")]
    check_compiled();
    #[cfg(feature = "async")]
    check_async();
    #[cfg(feature = "unified")]
    {
        let typed = TypedContainer::new(reexported_registry! {
            provide(StaticApp, instance(23u32)),
            provide(StaticRequest, |value: Inject<u32>| Ok::<_, InstantiateErrorKind>(value.0.to_string())),
        });
        assert_eq!(*typed.clone().get::<u32>().unwrap(), 23);
        let typed = typed.enter().with_scope(StaticRequest).build().unwrap();
        assert_eq!(typed.get_transient::<String>().unwrap(), "23");
        assert_eq!(&*typed.into_container().get::<String>().unwrap(), "23");

        let container = Container::new(reexported_registry! { provide(App, instance(9u32)) });
        assert_eq!(*container.get::<u32>().unwrap(), 9);

        type AppAlias = StaticApp;
        fn app() -> AppAlias {
            StaticApp
        }

        let fragment = reexported_registry! { provide(app(), instance(13u32)) };
        let container = Container::new(reexported_registry! {
            provide(StaticRequest, |value: Inject<u32>| Ok::<_, InstantiateErrorKind>(value.0.to_string())),
            extend(fragment),
        })
        .enter()
        .with_scope(StaticRequest)
        .build()
        .unwrap();
        assert_eq!(&*container.get::<String>().unwrap(), "13");

        fn service(value: Inject<u32>) -> Result<String, InstantiateErrorKind> {
            Ok(value.0.to_string())
        }

        let erased = reexported_registry! {
            provide(StaticApp, service),
            provide(StaticRequest, instance(7u32)),
        }
        .into_registry();
        let container = Container::new(dynamic_registry! {
            extend(erased, dynamic_registry! { provide(App, instance(19u32)) }),
        });
        assert_eq!(&*container.get::<String>().unwrap(), "19");

        #[cfg(feature = "async")]
        RuntimeBuilder::new_current_thread().build().unwrap().block_on(async {
            let typed = AsyncTypedContainer::new(renamed_async_registry! {
                provide(StaticApp, async || Ok::<_, InstantiateErrorKind>(29u32)),
            });
            assert_eq!(*typed.clone().get::<u32>().await.unwrap(), 29);
            assert_eq!(typed.get_transient::<u32>().await.unwrap(), 29);
            typed.close().await;

            let container = AsyncContainer::new(renamed_async_registry! {
                provide(StaticApp, async || Ok::<_, InstantiateErrorKind>(17u32)),
                provide(StaticRequest, async |value: Inject<u32>| Ok::<_, InstantiateErrorKind>(value.0.to_string())),
            })
            .enter()
            .with_scope(StaticRequest)
            .build()
            .unwrap();
            assert_eq!(&*container.get::<String>().await.unwrap(), "17");
        });
    }
}
