use di::{
    instance, registry as dynamic_registry, Container, DefaultScope::App, DependencyResolver, InstantiateErrorKind, ResolveErrorKind, Scope,
};
#[cfg(feature = "compiled")]
use di::{DefaultScope, Inject, InjectTransient};

struct Custom(u32);

impl DependencyResolver for Custom {
    type Error = ResolveErrorKind;

    fn resolve(container: &Container) -> Result<Self, Self::Error> {
        container.get::<u32>().map(|value| Self(*value))
    }

    #[cfg(feature = "async")]
    async fn resolve_async(container: &di::async_impl::Container) -> Result<Self, Self::Error> {
        container.get::<u32>().await.map(|value| Self(*value))
    }
}

fn dynamic(value: Custom) -> Result<String, InstantiateErrorKind> {
    Ok(value.0.to_string())
}

fn scoped<S: Scope>(registry: di::Registry, scope: S) -> Container {
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
    use di::{compiled_registry as registry, RuntimeDependency};

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
fn compiled_fragment() -> di::Registry {
    use di::compiled_registry as registry;

    registry! { provide(App, instance(11u32)) }.into_registry()
}

#[cfg(feature = "compiled")]
struct CycleLeft;
#[cfg(feature = "compiled")]
struct CycleRight;

#[cfg(feature = "compiled")]
fn cyclic_fragment() -> di::Registry {
    use di::compiled_registry as registry;

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
    assert!(std::panic::catch_unwind(|| Container::new(cyclic_fragment())).is_err());
}

#[cfg(all(feature = "compiled", feature = "async"))]
fn compiled_async_fragment() -> di::async_impl::RegistryWithSync {
    use di::compiled_async_registry as async_registry;

    async_registry! {
        provide(App, async || Ok(13u32)),
        provide(App, async |number: Inject<u32>| Ok(number.0.to_string())),
    }
    .into_async_registry()
}

#[cfg(all(feature = "compiled", feature = "async"))]
fn cyclic_async_fragment() -> di::async_impl::RegistryWithSync {
    use di::compiled_async_registry as async_registry;

    async_registry! {
        provide(App, async |_: Inject<CycleRight>| Ok(CycleLeft)),
        provide(App, async |_: Inject<CycleLeft>| Ok(CycleRight)),
    }
    .into_async_registry()
}

#[cfg(feature = "async")]
fn check_async() {
    use di::async_impl::Container;
    use di::async_registry as dynamic_async_registry;
    use di::{DefaultScope::Request, Inject};

    fn scoped<S: Scope + Clone>(registry: di::async_impl::RegistryWithSync, scope: S) -> Container {
        Container::new_with_start_scope::<S>(registry, scope)
    }

    tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
        let dynamic = dynamic_async_registry! {
            provide(App, async || Ok::<_, InstantiateErrorKind>(1u32)),
            provide(App, async || Ok::<_, InstantiateErrorKind>(7u32)),
            provide(App, async |custom: Custom| Ok::<_, InstantiateErrorKind>(custom.0.to_string())),
        };
        assert_eq!(
            *Container::new_with_start_scope(dynamic.clone(), App).get::<u32>().await.unwrap(),
            7
        );
        assert_eq!(&*scoped(dynamic, App).get::<String>().await.unwrap(), "7");

        struct NativeService;

        let container = Container::new(dynamic_async_registry! {
            provide(App, async |_: Inject<u16>| Ok::<_, InstantiateErrorKind>(NativeService)),
            extend(dynamic_registry! { provide(Request, instance(1u16)) }),
        });
        assert!(container.get::<NativeService>().await.is_err());

        #[cfg(feature = "compiled")]
        {
            use di::{compiled_async_registry as async_registry, RuntimeDependency};

            let container = Container::new_compiled_with_start_scope::<DefaultScope, _>(
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

            let container = Container::new(dynamic_async_registry! {
                provide(App, async |text: Inject<String>| Ok::<_, InstantiateErrorKind>(text.0.len())),
                extend(compiled_async_fragment()),
            });
            assert_eq!(*container.get::<usize>().await.unwrap(), 2);

            let container = Container::new(dynamic_async_registry! {
                extend(cyclic_async_fragment(), dynamic_async_registry! {
                    provide(App, async || Ok::<_, InstantiateErrorKind>(CycleLeft)),
                }),
            });
            assert!(container.get::<CycleRight>().await.is_ok());
            assert!(std::panic::catch_unwind(|| Container::new(cyclic_async_fragment())).is_err());
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
        use compiled_enabler::registry as reexported_registry;

        let container = Container::new(reexported_registry! { provide(App, instance(9u32)) });
        assert_eq!(*container.get::<u32>().unwrap(), 9);
    }
}
