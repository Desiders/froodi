//! Runtime registries: registry fragments whose type is erased.
//!
//! `registry!{ ... }.into_runtime()` turns a fragment into a [`RuntimeRegistry`], which has a
//! nameable type and can be returned from functions and crates. rustc no longer sees its
//! registrations, so its dependency requests are linked by binding key (`TypeId`) when a
//! container is built, and a missing binding becomes a startup diagnostic instead of a compile
//! error.
//!
//! Execution is the indexed backend (level A): each dependency edge goes through the target's
//! construction function by registration id, and the value is checked with a `downcast`.
//!
//! Static code cannot see into a runtime registry: a static factory that depends on a type only a
//! runtime registry provides does not compile unless the composition declares that boundary.

use alloc::{boxed::Box, vec::Vec};
use core::{any::TypeId, slice};

use froodi_compile_core::{CompiledEdge, Registration};

use crate::{
    container::Container,
    dependency_resolver::DependencyResolver,
    errors::{InstantiatorErrorKind, ResolveErrorKind, TypeInfo},
    finalizer::MaybeFinalizer,
    graph::{CollectRuntime, Describe, Empty, Entry, Finalize, Link, Node, Reg, Size, Walk},
    inject::{Inject, InjectTransient},
    instantiator::Instantiator,
    registry::Registry,
    scope::ScopeData,
    thread_safety::{RcAnyThreadSafety, RcThreadSafety, SendSafety, SyncSafety},
};

/// A factory parameter resolved through the edges the graph compiler linked by key.
pub trait DepIndexed: Sized {
    /// # Errors
    /// Returns the error of resolving the dependency.
    fn resolve(container: &Container, edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind>;
}

fn next_edge<'a>(edges: &mut slice::Iter<'a, CompiledEdge>) -> &'a CompiledEdge {
    edges
        .next()
        .expect("the graph compiler links one edge per Inject/InjectTransient parameter")
}

impl<T: SendSafety + SyncSafety + 'static> DepIndexed for Inject<T> {
    fn resolve(container: &Container, edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind> {
        let value = container.shared(next_edge(edges).target.index())?;
        value.downcast::<T>().map(Inject).map_err(|value| ResolveErrorKind::IncorrectType {
            expected: TypeInfo::of::<T>(),
            actual: TypeInfo {
                name: "<unknown>",
                id: (*value).type_id(),
            },
        })
    }
}

impl<T: 'static> DepIndexed for InjectTransient<T> {
    fn resolve(container: &Container, edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind> {
        container.transient_at::<T>(next_edge(edges).target.index()).map(InjectTransient)
    }
}

impl<R: DependencyResolver> DepIndexed for R {
    fn resolve(container: &Container, _edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind> {
        R::resolve(container).map_err(Into::into)
    }
}

/// All parameters of a factory, resolved through linked edges.
pub trait DepsIndexed: Sized {
    /// # Errors
    /// Returns the first dependency error.
    fn resolve(container: &Container, edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind>;
}

macro_rules! impl_deps_indexed {
    ([$($dep:ident),*]) => {
        impl<$($dep: DepIndexed,)*> DepsIndexed for ($($dep,)*) {
            #[inline]
            #[allow(unused_variables)]
            fn resolve(container: &Container, edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind> {
                Ok(($($dep::resolve(container, edges)?,)*))
            }
        }
    };
}

all_the_tuples!(impl_deps_indexed);

impl<T, F, D, Fin> Reg<T, F, D, Fin>
where
    F: Instantiator<D, Provides = T>,
    D: DepsIndexed,
{
    fn construct_indexed(&self, container: &Container, index: usize) -> Result<T, ResolveErrorKind> {
        let edges = container.edges(index);
        let dependencies = D::resolve(container, &mut edges.iter())
            .map_err(|err| ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(err.into())))?;
        self.factory
            .clone()
            .instantiate(dependencies)
            .map_err(|err| ResolveErrorKind::Instantiator(InstantiatorErrorKind::Factory(err.into())))
    }
}

/// # Safety
/// `item` must point to a live `Reg<T, F, D, Fin>` registered at `index`.
unsafe fn construct_indexed<T, F, D, Fin>(
    _root: *const (),
    item: *const (),
    container: &Container,
    index: usize,
) -> Result<RcAnyThreadSafety, ResolveErrorKind>
where
    T: SendSafety + SyncSafety + 'static,
    F: Instantiator<D, Provides = T>,
    D: DepsIndexed,
{
    // SAFETY: guaranteed by the caller.
    let item = unsafe { &*item.cast::<Reg<T, F, D, Fin>>() };
    Ok(RcThreadSafety::new(item.construct_indexed(container, index)?) as RcAnyThreadSafety)
}

/// # Safety
/// As [`construct_indexed`]; `out` must be valid for writing a `T`.
unsafe fn transient_indexed<T, F, D, Fin>(
    _root: *const (),
    item: *const (),
    container: &Container,
    index: usize,
    out: *mut (),
) -> Result<(), ResolveErrorKind>
where
    F: Instantiator<D, Provides = T>,
    D: DepsIndexed,
{
    // SAFETY: guaranteed by the caller.
    let item = unsafe { &*item.cast::<Reg<T, F, D, Fin>>() };
    let value = item.construct_indexed(container, index)?;
    // SAFETY: guaranteed by the caller.
    unsafe { out.cast::<T>().write(value) };
    Ok(())
}

/// # Safety
/// `item` must point to a live `Reg<T, F, D, Fin>`, and `value` must have been provided by it.
unsafe fn finalize_indexed<T: 'static, F, D, Fin: MaybeFinalizer<T>>(item: *const (), value: RcAnyThreadSafety) {
    // SAFETY: guaranteed by the caller.
    let item = unsafe { &*item.cast::<Reg<T, F, D, Fin>>() };
    // SAFETY: guaranteed by the caller: the value came from this registration, which provides `T`.
    item.finalizer
        .finalize(unsafe { crate::thread_safety::downcast_unchecked::<T>(value) });
}

/// Collects the indexed construction entries of an unlinked tree, in declaration order.
pub trait IndexedWalk {
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn walk_indexed(&self, entries: &mut Vec<Entry>);
}

impl<T, F, D, Fin> IndexedWalk for Reg<T, F, D, Fin>
where
    T: SendSafety + SyncSafety + 'static,
    F: Instantiator<D, Provides = T>,
    D: DepsIndexed,
    Fin: MaybeFinalizer<T>,
{
    #[allow(private_interfaces)]
    fn walk_indexed(&self, entries: &mut Vec<Entry>) {
        entries.push(Entry {
            item: core::ptr::from_ref(self).cast(),
            construct: construct_indexed::<T, F, D, Fin>,
            transient: transient_indexed::<T, F, D, Fin>,
            finalize: finalize_indexed::<T, F, D, Fin>,
        });
    }
}

impl<A: IndexedWalk, B: IndexedWalk> IndexedWalk for Node<A, B> {
    #[allow(private_interfaces)]
    fn walk_indexed(&self, entries: &mut Vec<Entry>) {
        self.0.walk_indexed(entries);
        self.1.walk_indexed(entries);
    }
}

impl IndexedWalk for Empty {
    #[allow(private_interfaces)]
    fn walk_indexed(&self, _entries: &mut Vec<Entry>) {}
}

impl IndexedWalk for RuntimeNode {
    #[allow(private_interfaces)]
    fn walk_indexed(&self, _entries: &mut Vec<Entry>) {}
}

/// An erased registry tree: what a container needs from a runtime registry.
pub trait RuntimeTree: SendSafety + SyncSafety {
    #[doc(hidden)]
    fn describe_runtime(&self, out: &mut Vec<Registration<TypeId>>);
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn walk_runtime(&self, entries: &mut Vec<Entry>);
}

impl<Tree> RuntimeTree for Tree
where
    Tree: Describe + IndexedWalk + CollectRuntime + SendSafety + SyncSafety + 'static,
{
    fn describe_runtime(&self, out: &mut Vec<Registration<TypeId>>) {
        self.describe(out);
        let mut nested = Vec::new();
        self.collect_runtime(&mut nested);
        for tree in nested {
            tree.describe_runtime(out);
        }
    }

    #[allow(private_interfaces)]
    fn walk_runtime(&self, entries: &mut Vec<Entry>) {
        self.walk_indexed(entries);
        let mut nested = Vec::new();
        self.collect_runtime(&mut nested);
        for tree in nested {
            tree.walk_runtime(entries);
        }
    }
}

/// A registry fragment with an erased, nameable type. See the module docs.
pub struct RuntimeRegistry {
    pub(crate) tree: Box<dyn RuntimeTree>,
    pub(crate) scopes: Vec<ScopeData>,
}

impl RuntimeRegistry {
    /// Marks every registration of this registry as an explicit replacement: it takes over the
    /// other registration of its type, static or runtime, including the edges rustc linked to it.
    /// This is how a test overrides a registration; an unmarked second registration of a type is
    /// a duplicate.
    #[must_use]
    pub fn replacing(self) -> Self {
        Self {
            tree: Box::new(Replacing(self.tree)),
            scopes: self.scopes,
        }
    }
}

struct Replacing(Box<dyn RuntimeTree>);

impl RuntimeTree for Replacing {
    fn describe_runtime(&self, out: &mut Vec<Registration<TypeId>>) {
        let start = out.len();
        self.0.describe_runtime(out);
        for registration in &mut out[start..] {
            registration.replaces = true;
        }
    }

    #[allow(private_interfaces)]
    fn walk_runtime(&self, entries: &mut Vec<Entry>) {
        self.0.walk_runtime(entries);
    }
}

impl<Tree> Registry<Tree>
where
    Tree: Describe + IndexedWalk + CollectRuntime + SendSafety + SyncSafety + 'static,
{
    /// Erases the registry's type. Its registrations are then linked by key when a container is
    /// built, and execute through the indexed backend.
    #[must_use]
    pub fn into_runtime(self) -> RuntimeRegistry {
        RuntimeRegistry {
            tree: Box::new(self.tree),
            scopes: self.scopes,
        }
    }
}

/// A runtime registry inside a registry tree. It has no size and no `Has` impls, so static
/// code cannot see its registrations.
pub struct RuntimeNode(pub(crate) Box<dyn RuntimeTree>);

impl Size for RuntimeNode {
    const SIZE: usize = 0;
}

impl Describe for RuntimeNode {
    fn describe(&self, _out: &mut Vec<Registration<TypeId>>) {}
}

impl<Root> Link<Root, ()> for RuntimeNode {
    type Linked = Self;

    #[inline]
    fn link(self) -> Self {
        self
    }
}

impl<Root> Walk<Root> for RuntimeNode {
    #[allow(private_interfaces)]
    fn walk(&self, _entries: &mut Vec<Entry>) {}
}

impl CollectRuntime for RuntimeNode {
    fn collect_runtime<'a>(&'a self, out: &mut Vec<&'a dyn RuntimeTree>) {
        out.push(&*self.0);
    }
}

impl Finalize for RuntimeNode {
    unsafe fn finalize(&self, _value: RcAnyThreadSafety) {}
}

/// What `extend(...)` accepts: a static registry, whose tree joins the outer tree, or a runtime
/// registry, which joins it as a [`RuntimeNode`].
pub trait IntoFragment {
    type Tree;

    #[doc(hidden)]
    fn into_fragment(self) -> (Self::Tree, Vec<ScopeData>);
}

impl<Tree> IntoFragment for Registry<Tree> {
    type Tree = Tree;

    #[inline]
    fn into_fragment(self) -> (Tree, Vec<ScopeData>) {
        self.into_parts()
    }
}

impl IntoFragment for RuntimeRegistry {
    type Tree = RuntimeNode;

    #[inline]
    fn into_fragment(self) -> (RuntimeNode, Vec<ScopeData>) {
        (RuntimeNode(self.tree), self.scopes)
    }
}
