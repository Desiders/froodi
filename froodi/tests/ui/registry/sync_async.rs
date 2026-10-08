use froodi::{DefaultScope::App, Inject, InstantiateErrorKind};

fn main() {
    let sync = froodi::registry! {
        provide(App, |_: Inject<u32>| Ok::<_, InstantiateErrorKind>(true)),
    };
    let _ = froodi::async_impl::Container::new(froodi::registry! {
        provide(App, async || Ok::<_, InstantiateErrorKind>(1u32)),
        extend(sync),
    });
}
