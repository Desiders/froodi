include!("support/resolver_candidates.rs");

fn main() {
    // With no static provider, the blanket candidate silently accepts normal injection.
    assert_eq!(infer(PhantomData::<Empty>, Inject::<u32>(RcThreadSafety::new(7u32))), None);
}
