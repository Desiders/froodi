use froodi::{compiled_registry, DefaultScope::App};

fn inst() -> u32 {
    7
}

fn main() {
    let _ = compiled_registry! {
        provide(App, inst),
    };
}
