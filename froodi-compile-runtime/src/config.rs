/// Per-registration configuration, as in Froodi. `cache_provides` is independent of the scope.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub cache_provides: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { cache_provides: true }
    }
}
