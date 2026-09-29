use crate::errors::InstantiateErrorKind;

/// A factory: a function or closure whose parameters are its dependencies.
///
/// The trait has the same shape as Froodi's. `Deps` is the tuple of parameter types, so the
/// dependency structure of a factory is part of its type while the factory itself, including a
/// closure's captured environment, stays a runtime value.
pub trait Instantiator<Deps>: Clone + 'static {
    type Provides: 'static;
    type Error: Into<InstantiateErrorKind>;

    /// # Errors
    /// Returns the factory's own error.
    fn instantiate(&mut self, dependencies: Deps) -> Result<Self::Provides, Self::Error>;
}

macro_rules! impl_instantiator {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(non_snake_case)]
        impl<F, Response, Err, $($ty,)*> Instantiator<($($ty,)*)> for F
        where
            F: FnMut($($ty,)*) -> Result<Response, Err> + Clone + 'static,
            Response: 'static,
            Err: Into<InstantiateErrorKind>,
        {
            type Provides = Response;
            type Error = Err;

            #[inline]
            /// # Errors
    /// Returns the factory's own error.
    fn instantiate(&mut self, ($($ty,)*): ($($ty,)*)) -> Result<Self::Provides, Self::Error> {
                self($($ty,)*)
            }
        }
    };
}

all_the_tuples!(impl_instantiator);
