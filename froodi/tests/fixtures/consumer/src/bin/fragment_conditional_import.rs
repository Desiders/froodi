use di::{registry, Container};

fn main() {
    let _ = Container::new(registry! {
        #[cfg(any())]
        use scope_support::fragments::second::infrastructure as selected;
        #[cfg(not(any()))]
        use scope_support::fragments::first::infrastructure as selected;

        extend_fragment(selected!(scope_support::fragments::Settings(41))),
    });
}
