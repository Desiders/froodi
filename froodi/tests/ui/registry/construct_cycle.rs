use froodi::{Container, DefaultScope::App};
use froodi::utils::thread_safety::RcThreadSafety;

#[derive(froodi::Construct)]
struct First { next: RcThreadSafety<Second> }

#[derive(froodi::Construct)]
struct Second { next: RcThreadSafety<First> }

fn main() {
    let _ = Container::new(froodi::registry! {
        scope(App) [ provide::<First>() ],
        scope(App) [ provide::<Second>() ],
    });
}
