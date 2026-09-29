//! `registry!` builds a balanced typed registration tree.
//! Rust trait resolution links `Inject<T>` / `InjectTransient<T>` to registrations.
//! The resulting paths become `RegistrationId`s for indexed execution.

use alloc::vec::Vec;
use core::{any::TypeId, marker::PhantomData, slice};

use froodi_compile_core::{
    CompiledEdge, DependencyRequest, ExecutionKind, Origin, Registration, RegistrationId, RequestMode, Target, ValueSource,
};

use crate::{
    config::Config,
    container::Container,
    dependency_resolver::DependencyResolver,
    errors::{InstantiatorErrorKind, ResolveErrorKind},
    finalizer::MaybeFinalizer,
    inject::{Inject, InjectTransient},
    instantiator::Instantiator,
    scope::ScopeData,
    thread_safety::{downcast_unchecked, RcAnyThreadSafety, RcThreadSafety, SendSafety, SyncSafety},
};

pub struct Here;
pub struct L<Path>(PhantomData<Path>);
pub struct R<Path>(PhantomData<Path>);

pub struct Reg<Out, Inst, Deps, Fin> {
    pub(crate) instantiator: Inst,
    pub(crate) finalizer: Fin,
    pub(crate) meta: Meta,
    marker: PhantomData<fn() -> (Out, Deps)>,
}

#[derive(Clone, Copy)]
pub struct Meta {
    pub(crate) scope: ScopeData,
    pub(crate) source: ValueSource,
    pub(crate) origin: Origin,
    pub(crate) config: Config,
}

impl<Out, Inst, Deps, Fin> Reg<Out, Inst, Deps, Fin> {
    #[inline]
    pub(crate) const fn new(instantiator: Inst, finalizer: Fin, meta: Meta) -> Self {
        Self {
            instantiator,
            finalizer,
            meta,
            marker: PhantomData,
        }
    }
}

pub struct Empty;

impl Size for Empty {
    const SIZE: usize = 0;
}

impl Describe for Empty {
    fn describe(&self, _out: &mut Vec<Registration<TypeId>>) {}
}

impl<Root> Link<Root, ()> for Empty {
    type Linked = Empty;

    #[inline]
    fn link(self) -> Empty {
        self
    }
}

impl CollectExecutors for Empty {
    #[allow(private_interfaces)]
    fn collect_executors(&self, _executors: &mut Vec<RegistrationExecutor>) {}
}

pub struct Node<Left, Right>(pub Left, pub Right);

pub trait Size {
    const SIZE: usize;
}

pub trait DependencyMetadata {
    fn request() -> DependencyRequest<TypeId>;
}

impl<T: 'static> DependencyMetadata for Inject<T> {
    fn request() -> DependencyRequest<TypeId> {
        DependencyRequest {
            target: Target::Key(TypeId::of::<T>()),
            mode: RequestMode::Inject,
            type_name: core::any::type_name::<T>(),
        }
    }
}

impl<T: 'static> DependencyMetadata for InjectTransient<T> {
    fn request() -> DependencyRequest<TypeId> {
        DependencyRequest {
            target: Target::Key(TypeId::of::<T>()),
            mode: RequestMode::InjectTransient,
            type_name: core::any::type_name::<T>(),
        }
    }
}

impl<R: DependencyResolver + 'static> DependencyMetadata for R {
    fn request() -> DependencyRequest<TypeId> {
        DependencyRequest {
            target: Target::Key(TypeId::of::<R>()),
            mode: RequestMode::Resolver,
            type_name: core::any::type_name::<R>(),
        }
    }
}

pub trait DependenciesMetadata {
    fn requests() -> Vec<DependencyRequest<TypeId>>;
}

macro_rules! impl_dependencies_metadata {
    ([$($dep:ident),*]) => {
        impl<$($dep: DependencyMetadata,)*> DependenciesMetadata for ($($dep,)*) {
            fn requests() -> Vec<DependencyRequest<TypeId>> {
                alloc::vec![$($dep::request()),*]
            }
        }
    };
}

all_the_tuples!(impl_dependencies_metadata);

/// Declaration order must match executor collection.
pub trait Describe {
    fn describe(&self, out: &mut Vec<Registration<TypeId>>);
}

impl<Out: 'static, Inst, Deps: DependenciesMetadata, Fin: MaybeFinalizer<Out>> Describe for Reg<Out, Inst, Deps, Fin> {
    fn describe(&self, out: &mut Vec<Registration<TypeId>>) {
        out.push(Registration {
            key: TypeId::of::<Out>(),
            type_name: core::any::type_name::<Out>(),
            requests: Deps::requests(),
            scope: self.meta.scope.into(),
            cache_provides: self.meta.config.cache_provides,
            finalizer: Fin::PRESENT.then_some(ExecutionKind::Sync),
            execution: ExecutionKind::Sync,
            source: self.meta.source,
            replaces: false,
            origin: Some(self.meta.origin),
        });
    }
}

impl<Left: Describe, Right: Describe> Describe for Node<Left, Right> {
    fn describe(&self, out: &mut Vec<Registration<TypeId>>) {
        self.0.describe(out);
        self.1.describe(out);
    }
}

impl<Left: Size, Right: Size> Size for Node<Left, Right> {
    const SIZE: usize = Left::SIZE + Right::SIZE;
}

/// Omits instantiators, dependency tuples and finalizers from provider lookup.
pub trait RegistryIndex {
    type Index;
}

pub struct Provider<Out>(PhantomData<fn() -> Out>);

impl<Out> Size for Provider<Out> {
    const SIZE: usize = 1;
}
impl<Out> ProviderPath<Out, Here> for Provider<Out> {
    type Provider = Self;
    const INDEX: usize = 0;
}
impl<Out, Execution> SupportsExecution<Execution> for Provider<Out> {}

impl<Out, Inst, Deps, Fin> RegistryIndex for Reg<Out, Inst, Deps, Fin> {
    type Index = Provider<Out>;
}
impl<Left: RegistryIndex, Right: RegistryIndex> RegistryIndex for Node<Left, Right> {
    type Index = Node<Left::Index, Right::Index>;
}
impl RegistryIndex for Empty {
    type Index = Empty;
}
impl RegistryIndex for ContainerLeaf {
    type Index = Provider<Container>;
}
impl RegistryIndex for crate::runtime_registry::RuntimeNode {
    type Index = Empty;
}
impl<T> RegistryIndex for crate::boundary::ImportLeaf<T> {
    type Index = Provider<T>;
}
impl<T> RegistryIndex for crate::boundary::ContextLeaf<T> {
    type Index = Provider<T>;
}

/// rustc infers `Path`; missing or ambiguous providers fail trait resolution.
#[diagnostic::on_unimplemented(
    message = "no registration provides `{T}`",
    label = "`{T}` is requested here, but no `provide(...)` in the registry produces it",
    note = "register an instantiator or `instance(...)` that returns `{T}`, or `extend(...)` a registry that does"
)]
pub trait ProviderPath<T, Path> {
    type Provider;
    const INDEX: usize;
}

impl<T, Path, Left: ProviderPath<T, Path>, Right> ProviderPath<T, L<Path>> for Node<Left, Right> {
    type Provider = Left::Provider;
    const INDEX: usize = Left::INDEX;
}

impl<T, Path, Left: Size, Right: ProviderPath<T, Path>> ProviderPath<T, R<Path>> for Node<Left, Right> {
    type Provider = Right::Provider;
    const INDEX: usize = Left::SIZE + Right::INDEX;
}

pub struct LinkedInject<Path>(PhantomData<Path>);
pub struct LinkedInjectTransient<Path>(PhantomData<Path>);
pub struct ByResolver;
pub struct SyncExecution;
#[cfg(feature = "async")]
pub struct AsyncExecution;

#[diagnostic::on_unimplemented(
    message = "this registration cannot be constructed synchronously",
    label = "a sync instantiator depends on it",
    note = "a sync instantiator may not depend on an async registration; make the dependent instantiator async"
)]
pub trait SupportsExecution<Execution> {}

#[diagnostic::on_unimplemented(
    message = "no registration provides the instantiator parameter `{Self}`",
    label = "this registry has no `provide(...)` for the type inside `{Self}`",
    note = "register an instantiator or `instance(...)` for it, or `extend(...)` a registry that does"
)]
pub trait LinkDependency<Root, Path> {
    type Provider;
    fn request() -> DependencyRequest<TypeId>;
}

impl<Root, T: 'static, Path> LinkDependency<Root, LinkedInject<Path>> for Inject<T>
where
    Root: ProviderPath<T, Path>,
{
    type Provider = Root::Provider;

    fn request() -> DependencyRequest<TypeId> {
        let mut request = <Self as DependencyMetadata>::request();
        request.target = Target::Id(RegistrationId(u32::try_from(Root::INDEX).expect("too many registrations")));
        request
    }
}

impl<Root, T: 'static, Path> LinkDependency<Root, LinkedInjectTransient<Path>> for InjectTransient<T>
where
    Root: ProviderPath<T, Path>,
{
    type Provider = Root::Provider;

    fn request() -> DependencyRequest<TypeId> {
        let mut request = <Self as DependencyMetadata>::request();
        request.target = Target::Id(RegistrationId(u32::try_from(Root::INDEX).expect("too many registrations")));
        request
    }
}

impl<Root, Resolver: DependencyResolver + 'static> LinkDependency<Root, ByResolver> for Resolver {
    type Provider = ();

    fn request() -> DependencyRequest<TypeId> {
        <Self as DependencyMetadata>::request()
    }
}

pub trait LinkDependencies<Root, Links> {
    type Providers;

    fn requests() -> Vec<DependencyRequest<TypeId>>;
}

macro_rules! impl_link_dependencies {
    ([$($dep:ident $path:ident),*]) => {
        impl<Root, $($dep: LinkDependency<Root, $path>, $path,)*>
            LinkDependencies<Root, ($($path,)*)> for ($($dep,)*) {
            type Providers = ($($dep::Provider,)*);
            fn requests() -> Vec<DependencyRequest<TypeId>> {
                alloc::vec![$(<$dep as LinkDependency<Root, $path>>::request()),*]
            }
        }
    };
}

all_the_tuple_pairs!(impl_link_dependencies);

// Check execution after provider inference, so an async provider is not reported as missing.
macro_rules! impl_supports_execution {
    ([$($provider:ident),*]) => {
        impl<Execution, $($provider: SupportsExecution<Execution>,)*> SupportsExecution<Execution> for ($($provider,)*) {}
    };
}
all_the_tuples!(impl_supports_execution);

pub struct Linked<Out, Inst, Deps, Fin> {
    pub(crate) instantiator: Inst,
    pub(crate) finalizer: Fin,
    pub(crate) meta: Meta,
    pub(crate) dependencies: Vec<DependencyRequest<TypeId>>,
    marker: PhantomData<fn() -> (Out, Deps)>,
}

/// `Links` mirrors the tree shape and is inferred by rustc.
pub trait Link<Root, Links> {
    type Linked;

    fn link(self) -> Self::Linked;
}

impl<Root, Out, Inst, Deps, Fin, Links> Link<Root, Links> for Reg<Out, Inst, Deps, Fin>
where
    Deps: LinkDependencies<Root, Links>,
    Deps::Providers: SupportsExecution<SyncExecution>,
{
    type Linked = Linked<Out, Inst, Deps, Fin>;

    #[inline]
    fn link(self) -> Self::Linked {
        Linked {
            instantiator: self.instantiator,
            finalizer: self.finalizer,
            meta: self.meta,
            dependencies: Deps::requests(),
            marker: PhantomData,
        }
    }
}

impl<Root, Left: Link<Root, LLinks>, Right: Link<Root, RLinks>, LLinks, RLinks> Link<Root, (LLinks, RLinks)> for Node<Left, Right> {
    type Linked = Node<Left::Linked, Right::Linked>;

    #[inline]
    fn link(self) -> Self::Linked {
        Node(self.0.link(), self.1.link())
    }
}

pub trait ConstructRegistration {
    type Provides: 'static;

    fn construct(&self, container: &Container, edges: &[CompiledEdge]) -> Result<Self::Provides, ResolveErrorKind>;

    /// The reference-counted value for `get` semantics. A registration that stands for another
    /// one returns that registration's value instead of a new allocation.
    #[inline]
    fn construct_inject(&self, container: &Container, edges: &[CompiledEdge]) -> Result<RcAnyThreadSafety, ResolveErrorKind>
    where
        Self::Provides: SendSafety + SyncSafety,
    {
        Ok(RcThreadSafety::new(self.construct(container, edges)?) as RcAnyThreadSafety)
    }
}

impl<Out: 'static, Inst, Deps, Fin> ConstructRegistration for Linked<Out, Inst, Deps, Fin>
where
    Inst: Instantiator<Deps, Provides = Out>,
    Deps: ResolveLinkedDependencies,
{
    type Provides = Out;

    #[inline]
    fn construct(&self, container: &Container, edges: &[CompiledEdge]) -> Result<Out, ResolveErrorKind> {
        // SAFETY: dispatch supplies this registration's compiled parameter edges.
        let dependencies = unsafe { Deps::resolve(container, &mut edges.iter()) }
            .map_err(|err| ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(err.into())))?;
        self.instantiator
            .clone()
            .instantiate(dependencies)
            .map_err(|err| ResolveErrorKind::Instantiator(InstantiatorErrorKind::Factory(err.into())))
    }
}

pub trait ResolveLinkedDependency: Sized {
    /// # Safety
    /// The next edge must provide the exact type requested by this parameter.
    /// Custom resolvers consume no edge.
    unsafe fn resolve(container: &Container, edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind>;
}

/// # Safety
/// An edge must remain for the current Inject/InjectTransient parameter. Linking emits
/// one request per parameter, and compilation removes only custom resolver requests.
#[inline]
pub(crate) unsafe fn next_edge<'a>(edges: &mut slice::Iter<'a, CompiledEdge>) -> &'a CompiledEdge {
    // SAFETY: the caller consumes exactly the edges emitted for its dependency tuple.
    unsafe { edges.next().unwrap_unchecked() }
}

impl<T: SendSafety + SyncSafety + 'static> ResolveLinkedDependency for Inject<T> {
    #[inline]
    unsafe fn resolve(container: &Container, edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind> {
        // SAFETY: the caller supplies this parameter's next compiled edge.
        let value = container.get_at(unsafe { next_edge(edges) }.target.index())?;
        // SAFETY: linking proved this edge provides T; graph compilation preserves its type.
        Ok(Inject(unsafe { downcast_unchecked(value) }))
    }
}

impl<T: 'static> ResolveLinkedDependency for InjectTransient<T> {
    #[inline]
    unsafe fn resolve(container: &Container, edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind> {
        // SAFETY: the caller supplies the edge linked for this InjectTransient<T> parameter.
        unsafe { container.get_transient_unchecked::<T>(next_edge(edges).target.index()) }.map(InjectTransient)
    }
}

impl<Resolver: DependencyResolver> ResolveLinkedDependency for Resolver {
    #[inline]
    unsafe fn resolve(container: &Container, _edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind> {
        Resolver::resolve(container).map_err(Into::into)
    }
}

pub trait ResolveLinkedDependencies: Sized {
    /// # Safety
    /// Edges must match this dependency tuple in parameter order, excluding custom resolvers.
    unsafe fn resolve(container: &Container, edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind>;
}

macro_rules! impl_resolve_linked_dependencies {
    ([$($dep:ident),*]) => {
        impl<$($dep: ResolveLinkedDependency,)*> ResolveLinkedDependencies for ($($dep,)*) {
            #[inline]
            #[allow(unused_variables)]
            unsafe fn resolve(container: &Container, edges: &mut slice::Iter<'_, CompiledEdge>) -> Result<Self, ResolveErrorKind> {
                // SAFETY: each parameter consumes its edge in order; resolvers consume none.
                Ok(($(unsafe { $dep::resolve(container, edges) }?,)*))
            }
        }
    };
}

all_the_tuples!(impl_resolve_linked_dependencies);

/// Bridges typed registrations to `RegistrationId`-indexed execution.
///
/// # Safety invariants
///
/// - The boxed tree and owned runtime fragments stay live at stable addresses until all
///   executors and borrowing futures are gone. `Plan` drops executor tables before the tree.
/// - Registration pointers and erased functions remain paired with their exact concrete
///   types; construction receives that registration's compiled parameter edges.
/// - IDs consistently index graph nodes, executors, type metadata and cache slots. Redirects
///   select the entire target executor; graph transformations must preserve this correspondence.
/// - Compiled edges preserve parameter order, exact provided types and one edge per built-in
///   parameter, excluding custom resolvers. This permits unchecked edge iteration.
/// - Slot N holds only its registration's exact provided Rust type, justifying unchecked casts.
///   Runtime, replacement, import, context and ancestor-cache paths must preserve that type.
/// - Transient output is aligned, writable, uninitialized storage for the exact provided type.
///   Read it only after success. Async transient output is an owned box, checked on extraction.
/// - Finalizers receive only values produced by their matching registration, tracked under
///   the actual producing ID after redirects. Close/drop retain the plan during finalization.
/// - Async construction cannot outlive borrowed plan storage; finalizer futures own their
///   cloned finalizer and value. Thread-safe plans require Send + Sync trees and synchronized access.
pub(crate) struct RegistrationExecutor {
    pub(crate) registration: *const (),
    pub(crate) construct: unsafe fn(item: *const (), &Container, edges: &[CompiledEdge]) -> Result<RcAnyThreadSafety, ResolveErrorKind>,
    pub(crate) construct_transient:
        unsafe fn(item: *const (), &Container, edges: &[CompiledEdge], out: *mut ()) -> Result<(), ResolveErrorKind>,
    pub(crate) finalize: unsafe fn(item: *const (), value: RcAnyThreadSafety),
}

/// # Safety
/// `item` must point to a live `Item`, and `value` must have been provided by it.
unsafe fn finalize_erased<Item: Finalize>(item: *const (), value: RcAnyThreadSafety) {
    // SAFETY: `item` points to a live `Item`, and `value` came from that registration.
    unsafe { (*item.cast::<Item>()).finalize(value) };
}

pub trait Finalize {
    /// # Safety
    /// `value` must have been provided by this registration.
    unsafe fn finalize(&self, value: RcAnyThreadSafety);
}

impl<Out: 'static, Inst, Deps, Fin: MaybeFinalizer<Out>> Finalize for Linked<Out, Inst, Deps, Fin> {
    unsafe fn finalize(&self, value: RcAnyThreadSafety) {
        // SAFETY: the caller guarantees `value` came from this registration, which provides `Out`.
        self.finalizer.finalize(unsafe { downcast_unchecked::<Out>(value) });
    }
}

/// # Safety
/// As [`construct_erased`]; `out` must be valid for writing an `Item::Provides`.
unsafe fn transient_erased<Item>(
    item: *const (),
    container: &Container,
    edges: &[CompiledEdge],
    out: *mut (),
) -> Result<(), ResolveErrorKind>
where
    Item: ConstructRegistration,
{
    // SAFETY: the caller supplies the live Item paired with this executor.
    let item = unsafe { &*item.cast::<Item>() };
    let value = item.construct(container, edges)?;
    // SAFETY: `out` is aligned, writable uninitialized storage for `Item::Provides`.
    unsafe { out.cast::<Item::Provides>().write(value) };
    Ok(())
}

impl RegistrationExecutor {
    pub(crate) fn of<Item>(item: &Item) -> Self
    where
        Item: ConstructRegistration + Finalize,
        Item::Provides: SendSafety + SyncSafety,
    {
        Self {
            registration: core::ptr::from_ref(item).cast(),
            construct: construct_erased::<Item>,
            construct_transient: transient_erased::<Item>,
            finalize: finalize_erased::<Item>,
        }
    }
}

/// # Safety
/// `item` must point to a live `Item` whose compiled parameter `edges` belong to the container's plan.
unsafe fn construct_erased<Item>(
    item: *const (),
    container: &Container,
    edges: &[CompiledEdge],
) -> Result<RcAnyThreadSafety, ResolveErrorKind>
where
    Item: ConstructRegistration,
    Item::Provides: SendSafety + SyncSafety,
{
    // SAFETY: the caller supplies the live Item paired with this executor.
    let item = unsafe { &*item.cast::<Item>() };
    item.construct_inject(container, edges)
}

/// Collection order must match graph registration IDs.
pub trait CollectExecutors {
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn collect_executors(&self, executors: &mut alloc::vec::Vec<RegistrationExecutor>);
}

impl<Out, Inst, Deps, Fin> CollectExecutors for Linked<Out, Inst, Deps, Fin>
where
    Self: ConstructRegistration<Provides = Out>,
    Out: SendSafety + SyncSafety + 'static,
    Fin: MaybeFinalizer<Out>,
{
    #[allow(private_interfaces)]
    fn collect_executors(&self, executors: &mut alloc::vec::Vec<RegistrationExecutor>) {
        executors.push(RegistrationExecutor::of::<Self>(self));
    }
}

impl<Left: CollectExecutors, Right: CollectExecutors> CollectExecutors for Node<Left, Right> {
    #[allow(private_interfaces)]
    fn collect_executors(&self, executors: &mut alloc::vec::Vec<RegistrationExecutor>) {
        self.0.collect_executors(executors);
        self.1.collect_executors(executors);
    }
}

/// Never cached: storing the container in its own cache would keep it alive.
pub struct ContainerLeaf {
    pub(crate) scope: ScopeData,
}

impl<Root> Link<Root, ()> for ContainerLeaf {
    type Linked = Self;

    #[inline]
    fn link(self) -> Self {
        self
    }
}

impl ConstructRegistration for ContainerLeaf {
    type Provides = Container;

    #[inline]
    fn construct(&self, container: &Container, _edges: &[CompiledEdge]) -> Result<Container, ResolveErrorKind> {
        Ok(container.clone())
    }
}

impl Finalize for ContainerLeaf {
    unsafe fn finalize(&self, _value: RcAnyThreadSafety) {}
}

impl CollectExecutors for ContainerLeaf {
    #[allow(private_interfaces)]
    fn collect_executors(&self, executors: &mut Vec<RegistrationExecutor>) {
        executors.push(RegistrationExecutor::of::<Self>(self));
    }
}

impl Describe for ContainerLeaf {
    fn describe(&self, out: &mut Vec<Registration<TypeId>>) {
        out.push(Registration {
            key: TypeId::of::<Container>(),
            type_name: core::any::type_name::<Container>(),
            requests: Vec::new(),
            scope: self.scope.into(),
            cache_provides: false,
            finalizer: None,
            execution: ExecutionKind::Sync,
            source: ValueSource::Container,
            replaces: false,
            origin: None,
        });
    }
}

/// Runtime fragments are numbered after all static registrations, in tree order.
pub trait CollectRuntime {
    #[doc(hidden)]
    fn collect_runtime<'a>(&'a self, _out: &mut Vec<&'a dyn crate::runtime_registry::RuntimeTree>) {}
}

impl<Out, Inst, Deps, Fin> CollectRuntime for Reg<Out, Inst, Deps, Fin> {}
impl<Out, Inst, Deps, Fin> CollectRuntime for Linked<Out, Inst, Deps, Fin> {}
impl CollectRuntime for Empty {}
impl CollectRuntime for ContainerLeaf {}

impl<Left: CollectRuntime, Right: CollectRuntime> CollectRuntime for Node<Left, Right> {
    fn collect_runtime<'a>(&'a self, out: &mut Vec<&'a dyn crate::runtime_registry::RuntimeTree>) {
        self.0.collect_runtime(out);
        self.1.collect_runtime(out);
    }
}

impl<Out: 'static, Inst, Deps, Fin: MaybeFinalizer<Out>> CollectRegistrations for Linked<Out, Inst, Deps, Fin> {
    fn collect_registrations(&mut self, out: &mut Vec<Registration<TypeId>>) {
        out.push(Registration {
            key: TypeId::of::<Out>(),
            type_name: core::any::type_name::<Out>(),
            requests: core::mem::take(&mut self.dependencies),
            scope: self.meta.scope.into(),
            cache_provides: self.meta.config.cache_provides,
            finalizer: Fin::PRESENT.then_some(ExecutionKind::Sync),
            execution: ExecutionKind::Sync,
            source: self.meta.source,
            replaces: false,
            origin: Some(self.meta.origin),
        });
    }
}

/// Moves materialized requests into the IR before executor collection, avoiding retained copies.
pub trait CollectRegistrations {
    fn collect_registrations(&mut self, out: &mut Vec<Registration<TypeId>>);
}

impl<Left: CollectRegistrations, Right: CollectRegistrations> CollectRegistrations for Node<Left, Right> {
    fn collect_registrations(&mut self, out: &mut Vec<Registration<TypeId>>) {
        self.0.collect_registrations(out);
        self.1.collect_registrations(out);
    }
}

macro_rules! collect_described_registrations {
    ($($ty:ty $(, $param:ident)*;)*) => {
        $(impl<$($param: 'static,)*> CollectRegistrations for $ty {
            fn collect_registrations(&mut self, out: &mut Vec<Registration<TypeId>>) {
                self.describe(out);
            }
        })*
    };
}

collect_described_registrations! {
    Empty;
    ContainerLeaf;
    crate::runtime_registry::RuntimeNode;
    crate::boundary::ImportLeaf<T>, T;
    crate::boundary::ContextLeaf<T>, T;
}
