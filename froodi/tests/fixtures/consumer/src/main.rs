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
}

#[cfg(feature = "async")]
fn check_async() {
    use di::async_impl::Container;
    use di::async_registry as dynamic_async_registry;

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
