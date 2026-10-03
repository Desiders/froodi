use di::{declare, instance, registry as renamed_registry, Container, DefaultScope::App, Inject, InjectTransient, InstantiateErrorKind, Registry};

#[cfg(feature = "async")]
use di::{async_impl::Container as AsyncContainer, async_registry as renamed_async_registry};
#[cfg(feature = "async")]
use tokio::runtime::Builder;
#[cfg(feature = "external-scopes")]
use scope_support::{registry as reexported_registry, scopes::{App as StaticApp, Request as StaticRequest}};

fn service(number: Inject<u32>, fresh: InjectTransient<u32>) -> Result<String, InstantiateErrorKind> {
    Ok(format!("{}:{}", number.0, fresh.0))
}

fn fragment() -> Registry {
    renamed_registry! { provide(App, instance(7u32)) }.into_registry()
}

fn main() {
    let container = Container::new(renamed_registry! {
        provide(App, declare::<u32>()),
        provide(App, service),
        extend(fragment()),
    });
    assert_eq!(&*container.get::<String>().unwrap(), "7:7");

    #[cfg(feature = "external-scopes")]
    {
        let container = Container::new(reexported_registry! {
            provide(StaticApp, instance(11u32)),
            provide(StaticRequest, |value: Inject<u32>| Ok::<_, InstantiateErrorKind>(value.0.to_string())),
        });
        let request = container.enter().with_scope(StaticRequest).build().unwrap();
        assert_eq!(&*request.get::<String>().unwrap(), "11");
    }

    #[cfg(feature = "async")]
    Builder::new_current_thread().build().unwrap().block_on(async {
        let container = AsyncContainer::new(renamed_async_registry! {
            provide(App, async || Ok::<_, InstantiateErrorKind>(7u32)),
            provide(App, async |value: Inject<u32>| Ok::<_, InstantiateErrorKind>(value.0.to_string())),
        });
        assert_eq!(&*container.get::<String>().await.unwrap(), "7");
    });
}
