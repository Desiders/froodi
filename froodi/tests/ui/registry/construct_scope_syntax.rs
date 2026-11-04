use froodi::registry;

#[derive(froodi::Construct)]
struct Service;

fn main() {
    let _ = registry! { provide::<Service>() };
}
