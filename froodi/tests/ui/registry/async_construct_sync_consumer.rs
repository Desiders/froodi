use froodi::{async_impl::Container, registry, utils::thread_safety::RcThreadSafety, DefaultScope::App};

#[derive(froodi::Construct)]
#[di(async)]
struct Repository;

#[derive(froodi::Construct)]
struct Service {
    repository: RcThreadSafety<Repository>,
}

fn main() {
    let _ = Container::new(registry! {
        scope(App) [ provide::<Repository>(), provide::<Service>(), ],
    });
}
