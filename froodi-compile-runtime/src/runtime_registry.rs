//! Erased fragments link by `TypeId` at container construction and use indexed executors.
//! Missing providers become startup diagnostics. Static dependencies need explicit import boundaries.

use alloc::{boxed::Box, vec::Vec};
use core::{any::TypeId, slice};

use froodi_compile_core::{CompiledEdge, Registration};

use crate::{
    container::Container,
    dependency_resolver::DependencyResolver,
    errors::{InstantiatorErrorKind, ResolveErrorKind, TypeInfo},
    finalizer::MaybeFinalizer,
    graph::{CollectExecutors, CollectRuntime, Describe, Empty, Finalize, Link, Node, Reg, RegistrationExecutor, Size},
    inject::{Inject, InjectTransient},
    instantiator::Instantiator,
    registry::Registry,
    scope::ScopeData,
    thread_safety::{RcAnyThreadSafety, RcThreadSafety, SendSafety, SyncSafety},
};

pub trait ResolveRuntimeDependency: Sized {
    fn resolve(container: &Container, edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind>;
}

fn next_edge<'a>(edges: &mut slice::Iter<'a, CompiledEdge>) -> &'a CompiledEdge {
    edges
        .next()
        .expect("the graph compiler links one edge per Inject/InjectTransient parameter")
}

impl<T: SendSafety + SyncSafety + 'static> ResolveRuntimeDependency for Inject<T> {
    fn resolve(container: &Container, edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind> {
        let value = container.get_at(next_edge(edges).target.index())?;
        value.downcast::<T>().map(Inject).map_err(|value| ResolveErrorKind::IncorrectType {
            expected: TypeInfo::of::<T>(),
            actual: TypeInfo {
                name: "<unknown>",
                id: (*value).type_id(),
            },
        })
    }
}

impl<T: 'static> ResolveRuntimeDependency for InjectTransient<T> {
    fn resolve(container: &Container, edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind> {
        container
            .get_transient_at::<T>(next_edge(edges).target.index())
            .map(InjectTransient)
    }
}

impl<R: DependencyResolver> ResolveRuntimeDependency for R {
    fn resolve(container: &Container, _edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind> {
        R::resolve(container).map_err(Into::into)
    }
}

pub trait ResolveRuntimeDependencies: Sized {
    fn resolve(container: &Container, edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind>;
}

macro_rules! impl_resolve_runtime_dependencies {
    ([$($dep:ident),*]) => {
        impl<$($dep: ResolveRuntimeDependency,)*> ResolveRuntimeDependencies for ($($dep,)*) {
            #[inline]
            #[allow(unused_variables)]
            fn resolve(container: &Container, edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind> {
                Ok(($($dep::resolve(container, edges)?,)*))
            }
        }
    };
}

all_the_tuples!(impl_resolve_runtime_dependencies);

impl<Out, Inst, Deps, Fin> Reg<Out, Inst, Deps, Fin>
where
    Inst: Instantiator<Deps, Provides = Out>,
    Deps: ResolveRuntimeDependencies,
{
    fn construct_indexed(&self, container: &Container, index: usize) -> Result<Out, ResolveErrorKind> {
        let edges = container.edges(index);
        let dependencies = Deps::resolve(container, &mut edges.iter())
            .map_err(|err| ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(err.into())))?;
        self.instantiator
            .clone()
            .instantiate(dependencies)
            .map_err(|err| ResolveErrorKind::Instantiator(InstantiatorErrorKind::Factory(err.into())))
    }
}

/// # Safety
/// `item` must point to a live `Reg<Out, Inst, Deps, Fin>` registered at `index`.
unsafe fn construct_indexed<Out, Inst, Deps, Fin>(
    _root: *const (),
    item: *const (),
    container: &Container,
    index: usize,
) -> Result<RcAnyThreadSafety, ResolveErrorKind>
where
    Out: SendSafety + SyncSafety + 'static,
    Inst: Instantiator<Deps, Provides = Out>,
    Deps: ResolveRuntimeDependencies,
{
    // SAFETY: the caller supplies a live `Reg<Out, Inst, Deps, Fin>` with this executor's registration ID.
    let item = unsafe { &*item.cast::<Reg<Out, Inst, Deps, Fin>>() };
    Ok(RcThreadSafety::new(item.construct_indexed(container, index)?) as RcAnyThreadSafety)
}

/// # Safety
/// As [`construct_indexed`]; `out` must be valid for writing an `Out`.
unsafe fn transient_indexed<Out, Inst, Deps, Fin>(
    _root: *const (),
    item: *const (),
    container: &Container,
    index: usize,
    out: *mut (),
) -> Result<(), ResolveErrorKind>
where
    Inst: Instantiator<Deps, Provides = Out>,
    Deps: ResolveRuntimeDependencies,
{
    // SAFETY: the caller supplies a live `Reg<Out, Inst, Deps, Fin>` with this executor's registration ID.
    let item = unsafe { &*item.cast::<Reg<Out, Inst, Deps, Fin>>() };
    let value = item.construct_indexed(container, index)?;
    // SAFETY: `out` is aligned, writable uninitialized storage for `Out`.
    unsafe { out.cast::<Out>().write(value) };
    Ok(())
}

/// # Safety
/// `item` must point to a live `Reg<Out, Inst, Deps, Fin>`, and `value` must have been provided by it.
unsafe fn finalize_indexed<Out: 'static, Inst, Deps, Fin: MaybeFinalizer<Out>>(item: *const (), value: RcAnyThreadSafety) {
    // SAFETY: the caller supplies a live `Reg<Out, Inst, Deps, Fin>` with this executor's registration ID.
    let item = unsafe { &*item.cast::<Reg<Out, Inst, Deps, Fin>>() };
    // SAFETY: guaranteed by the caller: the value came from this registration, which provides `Out`.
    item.finalizer
        .finalize(unsafe { crate::thread_safety::downcast_unchecked::<Out>(value) });
}

/// Collection order must match `describe_runtime`.
pub trait CollectRuntimeExecutors {
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn collect_runtime_executors(&self, executors: &mut Vec<RegistrationExecutor>);
}

impl<Out, Inst, Deps, Fin> CollectRuntimeExecutors for Reg<Out, Inst, Deps, Fin>
where
    Out: SendSafety + SyncSafety + 'static,
    Inst: Instantiator<Deps, Provides = Out>,
    Deps: ResolveRuntimeDependencies,
    Fin: MaybeFinalizer<Out>,
{
    #[allow(private_interfaces)]
    fn collect_runtime_executors(&self, executors: &mut Vec<RegistrationExecutor>) {
        executors.push(RegistrationExecutor {
            registration: core::ptr::from_ref(self).cast(),
            construct: construct_indexed::<Out, Inst, Deps, Fin>,
            construct_transient: transient_indexed::<Out, Inst, Deps, Fin>,
            finalize: finalize_indexed::<Out, Inst, Deps, Fin>,
        });
    }
}

impl<Left: CollectRuntimeExecutors, Right: CollectRuntimeExecutors> CollectRuntimeExecutors for Node<Left, Right> {
    #[allow(private_interfaces)]
    fn collect_runtime_executors(&self, executors: &mut Vec<RegistrationExecutor>) {
        self.0.collect_runtime_executors(executors);
        self.1.collect_runtime_executors(executors);
    }
}

impl CollectRuntimeExecutors for Empty {
    #[allow(private_interfaces)]
    fn collect_runtime_executors(&self, _executors: &mut Vec<RegistrationExecutor>) {}
}

impl CollectRuntimeExecutors for RuntimeNode {
    #[allow(private_interfaces)]
    fn collect_runtime_executors(&self, _executors: &mut Vec<RegistrationExecutor>) {}
}

pub trait RuntimeTree: SendSafety + SyncSafety {
    #[doc(hidden)]
    fn describe_runtime(&self, out: &mut Vec<Registration<TypeId>>);
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn collect_fragment_executors(&self, executors: &mut Vec<RegistrationExecutor>);
}

impl<Tree> RuntimeTree for Tree
where
    Tree: Describe + CollectRuntimeExecutors + CollectRuntime + SendSafety + SyncSafety + 'static,
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
    fn collect_fragment_executors(&self, executors: &mut Vec<RegistrationExecutor>) {
        self.collect_runtime_executors(executors);
        let mut nested = Vec::new();
        self.collect_runtime(&mut nested);
        for tree in nested {
            tree.collect_fragment_executors(executors);
        }
    }
}

pub struct RuntimeRegistry {
    pub(crate) tree: Box<dyn RuntimeTree>,
    pub(crate) scopes: Vec<ScopeData>,
}

impl RuntimeRegistry {
    /// Overrides providers of the same type, including statically linked edges.
    /// Unmarked duplicate providers remain errors.
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
    fn collect_fragment_executors(&self, executors: &mut Vec<RegistrationExecutor>) {
        self.0.collect_fragment_executors(executors);
    }
}

impl<Tree> Registry<Tree>
where
    Tree: Describe + CollectRuntimeExecutors + CollectRuntime + SendSafety + SyncSafety + 'static,
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

/// A runtime registry inside a registry tree. It has no size and no `ProviderPath` impls, so static
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

impl<Root> CollectExecutors<Root> for RuntimeNode {
    #[allow(private_interfaces)]
    fn collect_executors(&self, _executors: &mut Vec<RegistrationExecutor>) {}
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
/// registry, which joins it as a `RuntimeNode`.
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
