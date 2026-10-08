use scope_support::fragments::private_fragment;
fn main() {
    let _ = di::registry! { extend_fragment(private_fragment!()), };
}
