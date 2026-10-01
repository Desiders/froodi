use froodi::{DefaultScope::App, InjectTransient, InstantiateErrorKind};

fn main() {
    let sync = froodi::compiled_registry! {
        provide(App, |_: InjectTransient<u32>| Ok::<_, InstantiateErrorKind>(true)),
    };
    let _ = froodi::async_impl::Container::new(froodi::compiled_async_registry! {
        provide(App, async || Ok::<_, InstantiateErrorKind>(1u32)),
        extend(sync),
    });
}
