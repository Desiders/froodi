use froodi::{registry, DefaultScope::*, InstantiateErrorKind};

fn main() {
    let _registry = registry! {
        scope(App) [
            provide(|| Ok::<_, InstantiateErrorKind>(1_u8)),
        ],
        frobnicate(App),
    };
}
