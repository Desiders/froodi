/// Config for an instantiator
/// ## Fields
/// - `cache_provides`:
///   If `true`, the instance provided by the instantiator will be cached and reused.
///
///   This does **not** affect the dependencies of the instance.
///   Only the final result is cached if caching is applicable.
#[derive(Clone, Copy)]
pub struct Config {
    pub cache_provides: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { cache_provides: true }
    }
}
