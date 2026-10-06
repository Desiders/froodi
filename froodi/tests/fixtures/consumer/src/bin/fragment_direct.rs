use di::{fragment, registry};

#[fragment(configuration(value))]
registry! {}

fn main() {
    let _ = configuration!(7u32);
}
