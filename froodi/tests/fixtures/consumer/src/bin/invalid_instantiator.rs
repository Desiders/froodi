use di::{registry, DefaultScope::App};

fn inst() -> u32 {
    7
}

fn main() {
    let _ = registry! {
        provide(App, inst),
    };
}
