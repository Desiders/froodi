use froodi::{registry, DefaultScope::App};

#[derive(froodi::Construct)]
struct Service;

fn main() {
    let _ = registry! { provide::<Service>(App) };
}
