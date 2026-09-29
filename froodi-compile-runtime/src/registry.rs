use alloc::vec::Vec;
use core::any::TypeId;

use froodi_compile_core::{Graph, Origin, ValueSource};

use crate::{
    boundary::Provide,
    config::Config,
    graph::{CollectRuntime, Describe, Empty, Meta},
    scope::{DefaultScope, Scope, ScopeData, Scopes},
};

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
        Tree: Describe + CollectRuntime,
    {
        let mut graph = Graph::new(self.scopes.iter().copied().map(Into::into).collect());
        self.tree.describe(&mut graph.registrations);
        let mut runtime = Vec::new();
        self.tree.collect_runtime(&mut runtime);
        for tree in runtime {
            tree.describe_runtime(&mut graph.registrations);
        }
        graph
    }
}

#[doc(hidden)]
#[inline]
pub fn reg<S: Scope, P, Deps, Fin>(
    scope: S,
    provider: P,
    config: Option<Config>,
    finalizer: Fin,
    source: ValueSource,
    origin: Origin,
) -> P::Leaf
where
    P: Provide<Deps, Fin>,
{
    provider.into_leaf(
        finalizer,
        Meta {
            scope: scope.into(),
            source,
            origin,
            config: config.unwrap_or_default(),
        },
    )
}
