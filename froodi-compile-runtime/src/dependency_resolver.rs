use crate::{container::Container, errors::ResolveErrorKind};

/// A factory parameter resolved by user code that reads the container at runtime, as in Froodi.
///
/// Its dependencies are invisible to the graph compiler: the registration IR records the request
/// as [`RequestMode::Resolver`](froodi_compile_core::RequestMode::Resolver) and no edge is built.
/// `Inject<T>` and `InjectTransient<T>` are not resolvers; they are static edges.
pub trait DependencyResolver: Sized {
    type Error: Into<ResolveErrorKind>;

    /// # Errors
    /// Returns the resolver's own error.
    fn resolve(container: &Container) -> Result<Self, Self::Error>;
}
