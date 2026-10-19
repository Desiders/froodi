use froodi::{async_impl::Container, DefaultScope::App, InstantiateErrorKind};
use froodi::utils::thread_safety::RcThreadSafety;

#[derive(froodi::Construct)]
struct Service { dependency: RcThreadSafety<u32> }

fn main() {
    let _ = Container::new(froodi::registry! {
        scope(App) [ provide::<Service>() ],
        provide(App, async || Ok::<_, InstantiateErrorKind>(7u32)),
    });
}
