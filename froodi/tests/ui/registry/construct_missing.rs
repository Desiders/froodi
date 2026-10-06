use froodi::{Container, DefaultScope::App};
use froodi::utils::thread_safety::RcThreadSafety;

#[derive(froodi::Construct)]
struct Service { dependency: RcThreadSafety<u32> }

fn main() {
    let _ = Container::new(froodi::registry! { scope(App) [ provide::<Service>() ] });
}
