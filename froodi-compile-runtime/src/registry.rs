use crate::{graph::Reg, instantiator::Instantiator, scope::Scope};

/// A registry fragment: the typed registration tree built by `registry!`.
pub struct Registry<Tree> {
    pub(crate) tree: Tree,
}

impl<Tree> Registry<Tree> {
    #[doc(hidden)]
    #[inline]
    pub const fn from_tree(tree: Tree) -> Self {
        Self { tree }
    }
}

/// One `provide(...)` item.
#[doc(hidden)]
#[inline]
pub fn reg<S: Scope, F, D>(_scope: S, factory: F) -> Reg<F::Provides, F, D>
where
    F: Instantiator<D>,
{
    Reg::new(factory)
}
