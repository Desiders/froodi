use froodi::{compiled_async_registry, DefaultScope::App};

async fn inst() -> u32 {
    7
}

fn main() {
    let _ = compiled_async_registry! {
        provide(App, inst),
    };
}
