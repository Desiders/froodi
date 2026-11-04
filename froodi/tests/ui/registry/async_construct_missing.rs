use froodi::{async_impl::Container, registry, utils::thread_safety::RcThreadSafety, DefaultScope::App};

#[derive(froodi::Construct)]
#[di(async)]
struct Service {
    number: RcThreadSafety<u32>,
}

fn main() {
    let _ = Container::new(registry! {
        scope(App) [ provide::<Service>(), ],
    });
}
