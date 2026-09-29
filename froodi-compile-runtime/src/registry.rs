use alloc::vec::Vec;
use core::any::TypeId;

use froodi_compile_core::{Graph, Origin, ValueSource};

use crate::{
    config::Config,
    finalizer::MaybeFinalizer,
    graph::{Describe, Empty, Meta, Reg},
    instantiator::Instantiator,
    scope::{DefaultScope, Scope, ScopeData, Scopes},
};

/// A registry fragment: the typed registration tree built by `registry!`, plus the scope
/// hierarchy its scopes belong to.
pub struct Registry<Tree> {
    pub(crate) tree: Tree,
    pub(crate) scopes: Vec<ScopeData>,
}

impl Registry<Empty> {
    /// `registry!()`: no registrations, the `DefaultScope` hierarchy.
    #[doc(hidden)]
    #[must_use]
    pub fn empty() -> Self {
        Self::from_tree::<DefaultScope, 5, 0>(Empty, [])
    }
}

impl<Tree> Registry<Tree> {
    /// `scopes` are the scope values the registry uses; they fix the scope type `S` whose
    /// `Scopes::all()` is the hierarchy.
    #[doc(hidden)]
    #[inline]
    pub fn from_tree<S, const N: usize, const M: usize>(tree: Tree, _scopes: [S; M]) -> Self
    where
        S: Scope + Scopes<N, Scope = S>,
    {
        let (root, children) = S::all();
        let mut scopes = Vec::with_capacity(N + 1);
        scopes.push(root.into());
        scopes.extend(children.into_iter().map(Into::into));
        Self { tree, scopes }
    }

    #[doc(hidden)]
    #[inline]
    pub fn into_parts(self) -> (Tree, Vec<ScopeData>) {
        (self.tree, self.scopes)
    }

    #[doc(hidden)]
    #[inline]
    pub const fn from_parts(tree: Tree, scopes: Vec<ScopeData>) -> Self {
        Self { tree, scopes }
    }

    /// The registration IR of this registry. Dependency requests carry binding keys; resolving
    /// them is the graph compiler's job.
    #[must_use]
    pub fn graph(&self) -> Graph<TypeId>
    where
        Tree: Describe,
    {
        let mut graph = Graph::new(self.scopes.iter().copied().map(Into::into).collect());
        self.tree.describe(&mut graph.registrations);
        graph
    }
}

/// One `provide(...)` item.
#[doc(hidden)]
#[inline]
pub fn reg<S: Scope, F, D, Fin>(
    scope: S,
    factory: F,
    config: Option<Config>,
    finalizer: Fin,
    source: ValueSource,
    origin: Origin,
) -> Reg<F::Provides, F, D, Fin>
where
    F: Instantiator<D>,
    Fin: MaybeFinalizer<F::Provides>,
{
    Reg::new(
        factory,
        finalizer,
        Meta {
            scope: scope.into(),
            source,
            origin,
            config: config.unwrap_or_default(),
        },
    )
}
