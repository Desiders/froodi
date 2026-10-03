use di::{async_registry, DefaultScope::App};

async fn inst() -> u32 {
    7
}

fn main() {
    let _ = async_registry! {
        provide(App, inst),
    };
}
