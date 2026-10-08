use di::utils::thread_safety::RcThreadSafety;
use di::{fragment, registry, DefaultScope::App, InstantiateErrorKind};

#[derive(di::Construct)]
struct Service {
    database: RcThreadSafety<u32>,
}
async fn database() -> Result<u32, InstantiateErrorKind> {
    Ok(7)
}
#[fragment(first)]
registry! { scope(App) [ provide::<Service>(), ], }
#[fragment(second)]
registry! { provide(App, database), }

fn main() {
    let _ = di::async_impl::Container::new(registry! { extend_fragment(first!(), second!()), });
}
