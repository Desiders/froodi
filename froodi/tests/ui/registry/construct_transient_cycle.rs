use froodi::{Container, DefaultScope::App};
use froodi::utils::thread_safety::RcThreadSafety;

#[derive(froodi::Construct)]
struct First {
    #[di(inject_transient)]
    next: Second,
}

#[derive(froodi::Construct)]
struct Second { next: RcThreadSafety<First> }

fn main() {
    let _ = Container::new(froodi::registry! {
        scope(App) [ provide::<First>(), provide::<Second>(), ],
    });
}
