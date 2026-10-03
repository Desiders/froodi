include!("support/resolver_candidates.rs");

fn main() {
    // Even a present provider now has two candidates: indexed injection and ordinary resolver.
    let _ = infer(PhantomData::<Provider<u32>>, Inject::<u32>(RcThreadSafety::new(7u32)));
}
