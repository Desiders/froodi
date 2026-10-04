use froodi::{Container, DefaultScope::App};

#[derive(froodi::Construct)]
struct Bad<T> { values: Vec<T> }

fn main() {
    let _ = Container::new(froodi::registry! { scope(App) [ construct::<Bad<u32>>() ] });
}
