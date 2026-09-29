use crate::errors::InstantiateErrorKind;

/// `Deps` preserves dependency types while captured instantiator state remains a runtime value.
pub trait Instantiator<Deps>: Clone + 'static {
    type Provides: 'static;
    type Error: Into<InstantiateErrorKind>;

    fn instantiate(&mut self, dependencies: Deps) -> Result<Self::Provides, Self::Error>;
}

macro_rules! impl_instantiator {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(non_snake_case)]
        impl<Inst, Out, Err, $($ty,)*> Instantiator<($($ty,)*)> for Inst
        where
            Inst: FnMut($($ty,)*) -> Result<Out, Err> + Clone + 'static,
            Out: 'static,
            Err: Into<InstantiateErrorKind>,
        {
            type Provides = Out;
            type Error = Err;

            #[inline]
            fn instantiate(&mut self, ($($ty,)*): ($($ty,)*)) -> Result<Self::Provides, Self::Error> {
                self($($ty,)*)
            }
        }
    };
}

all_the_tuples!(impl_instantiator);

/// Clones the supplied value on each instantiation.
#[inline]
#[must_use]
pub const fn instance<T: Clone + 'static>(val: T) -> impl Instantiator<(), Provides = T, Error = InstantiateErrorKind> {
    move || Ok(val.clone())
}
