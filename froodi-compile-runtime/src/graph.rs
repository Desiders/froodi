//! `registry!` builds a balanced typed registration tree.
//! Rust trait resolution links `Inject<T>` / `InjectTransient<T>` to registrations.
//! The resulting paths become `RegistrationId`s for indexed execution.

use alloc::vec::Vec;
use core::{any::TypeId, marker::PhantomData};

use froodi_compile_core::{DependencyRequest, ExecutionKind, Origin, Registration, RegistrationId, RequestMode, Target, ValueSource};

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

impl<Root> CollectExecutors<Root> for Empty {
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

impl<Out, Inst, Deps, Fin> Size for Reg<Out, Inst, Deps, Fin> {
    const SIZE: usize = 1;
}

impl<Left: Size, Right: Size> Size for Node<Left, Right> {
    const SIZE: usize = Left::SIZE + Right::SIZE;
}

/// rustc infers `Path`; missing or ambiguous providers fail trait resolution.
#[diagnostic::on_unimplemented(
    message = "no registration provides `{T}`",
    label = "`{T}` is requested here, but no `provide(...)` in the registry produces it",
    note = "register an instantiator or `instance(...)` that returns `{T}`, or `extend(...)` a registry that does"
)]
pub trait ProviderPath<T, Path> {}

impl<Out, Inst, Deps, Fin> ProviderPath<Out, Here> for Reg<Out, Inst, Deps, Fin> {}

impl<T, Path, Left: ProviderPath<T, Path>, Right> ProviderPath<T, L<Path>> for Node<Left, Right> {}

impl<T, Path, Left, Right: ProviderPath<T, Path>> ProviderPath<T, R<Path>> for Node<Left, Right> {}

/// Separate from provider lookup to avoid duplicate missing-binding errors during metadata collection.
pub trait RegistrationPath<Path> {
    type Registration;
    const INDEX: usize;
}

impl<Out, Inst, Deps, Fin, Links> RegistrationPath<Here> for Linked<Out, Inst, Deps, Fin, Links> {
    type Registration = Self;
    const INDEX: usize = 0;
}

impl<Path, Left: RegistrationPath<Path>, Right> RegistrationPath<L<Path>> for Node<Left, Right> {
    type Registration = Left::Registration;
    const INDEX: usize = Left::INDEX;
}

impl<Path, Left: Size, Right: RegistrationPath<Path>> RegistrationPath<R<Path>> for Node<Left, Right> {
    type Registration = Right::Registration;
    const INDEX: usize = Left::SIZE + Right::INDEX;
}

pub struct LinkedInject<Path>(PhantomData<Path>);

#[diagnostic::on_unimplemented(
    message = "no registration provides the instantiator parameter `{Self}`",
    label = "this registry has no `provide(...)` for the type inside `{Self}`",
    note = "register an instantiator or `instance(...)` for it, or `extend(...)` a registry that does"
)]
pub trait LinkDependency<Root, Path> {}

impl<Root: ProviderPath<T, Path>, T, Path> LinkDependency<Root, LinkedInject<Path>> for Inject<T> {}

pub struct LinkedInjectTransient<Path>(PhantomData<Path>);

impl<Root: ProviderPath<T, Path>, T, Path> LinkDependency<Root, LinkedInjectTransient<Path>> for InjectTransient<T> {}

/// Custom resolvers have no static dependency edge.
pub struct ByResolver;

impl<Root, R: DependencyResolver> LinkDependency<Root, ByResolver> for R {}

pub trait LinkDependencies<Root, Links> {}

macro_rules! impl_link_dependencies {
    ([$($dep:ident $index:ident),*]) => {
        impl<Root, $($dep: LinkDependency<Root, $index>, $index,)*> LinkDependencies<Root, ($($index,)*)> for ($($dep,)*) {}
    };
}

all_the_tuple_pairs!(impl_link_dependencies);

pub struct Linked<Out, Inst, Deps, Fin, Links> {
    pub(crate) instantiator: Inst,
    pub(crate) finalizer: Fin,
    pub(crate) meta: Meta,
    #[allow(clippy::type_complexity, reason = "keep the registration marker inline with its type parameters")]
    marker: PhantomData<fn() -> (Out, Deps, Links)>,
}

impl<Out, Inst, Deps, Fin, Links> Size for Linked<Out, Inst, Deps, Fin, Links> {
    const SIZE: usize = 1;
}

/// `Links` mirrors the tree shape and is inferred by rustc.
pub trait Link<Root, Links> {
    type Linked;

    fn link(self) -> Self::Linked;
}

impl<Root, Out, Inst, Deps, Fin, Links> Link<Root, Links> for Reg<Out, Inst, Deps, Fin>
where
    Deps: LinkDependencies<Root, Links>,
{
    type Linked = Linked<Out, Inst, Deps, Fin, Links>;

    #[inline]
    fn link(self) -> Self::Linked {
        Linked {
            instantiator: self.instantiator,
            finalizer: self.finalizer,
            meta: self.meta,
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

#[diagnostic::on_unimplemented(
    message = "this registration cannot be constructed synchronously",
    label = "a sync instantiator depends on it",
    note = "a sync instantiator may not depend on an async registration; make the dependent instantiator async"
)]
pub trait ConstructRegistration<Root> {
    type Provides: 'static;

    fn construct(&self, root: &Root, container: &Container, index: usize) -> Result<Self::Provides, ResolveErrorKind>;

    /// The reference-counted value for `get` semantics. A registration that stands for another
    /// one returns that registration's value instead of a new allocation.
    #[inline]
    fn construct_inject(&self, root: &Root, container: &Container, index: usize) -> Result<RcAnyThreadSafety, ResolveErrorKind>
    where
        Self::Provides: SendSafety + SyncSafety,
    {
        Ok(RcThreadSafety::new(self.construct(root, container, index)?) as RcAnyThreadSafety)
    }
}

impl<Root, Out: 'static, Inst, Deps, Fin, Links> ConstructRegistration<Root> for Linked<Out, Inst, Deps, Fin, Links>
where
    Inst: Instantiator<Deps, Provides = Out>,
    Deps: ResolveLinkedDependencies<Root, Links>,
{
    type Provides = Out;

    #[inline]
    fn construct(&self, root: &Root, container: &Container, _index: usize) -> Result<Out, ResolveErrorKind> {
        let dependencies =
            Deps::resolve(root, container).map_err(|err| ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(err.into())))?;
        self.instantiator
            .clone()
            .instantiate(dependencies)
            .map_err(|err| ResolveErrorKind::Instantiator(InstantiatorErrorKind::Factory(err.into())))
    }
}

#[diagnostic::on_unimplemented(
    message = "no registration provides the instantiator parameter `{Self}`",
    label = "this registry has no `provide(...)` for the type inside `{Self}`"
)]
pub trait ResolveLinkedDependency<Root, Path>: Sized {
    fn resolve(root: &Root, container: &Container) -> Result<Self, ResolveErrorKind>;
}

/// Shallow bounds keep dependency graph depth independent of trait-resolution depth.
#[diagnostic::on_unimplemented(
    message = "this registration cannot be constructed synchronously",
    label = "a sync instantiator depends on it",
    note = "a sync instantiator may not depend on an async registration; make the dependent instantiator async"
)]
pub trait SyncProvider<T> {}

impl<Out, Inst, Deps, Fin, Links> SyncProvider<Out> for Linked<Out, Inst, Deps, Fin, Links> {}
impl SyncProvider<Container> for ContainerLeaf {}
impl<T> SyncProvider<T> for crate::boundary::ImportLeaf<T> {}
impl<T> SyncProvider<T> for crate::boundary::ContextLeaf<T> {}

impl<Root, T: SendSafety + SyncSafety + 'static, Path> ResolveLinkedDependency<Root, LinkedInject<Path>> for Inject<T>
where
    Root: RegistrationPath<Path>,
    Root::Registration: SyncProvider<T>,
{
    #[inline]
    fn resolve(_root: &Root, container: &Container) -> Result<Self, ResolveErrorKind> {
        let value = container.get_at(Root::INDEX)?;
        // SAFETY: slot `Root::INDEX` only ever holds values of the registration at path `Path`,
        // which provides `T`.
        Ok(Inject(unsafe { downcast_unchecked(value) }))
    }
}

impl<Root, T: 'static, Path> ResolveLinkedDependency<Root, LinkedInjectTransient<Path>> for InjectTransient<T>
where
    Root: RegistrationPath<Path>,
    Root::Registration: SyncProvider<T>,
{
    #[inline]
    fn resolve(_root: &Root, container: &Container) -> Result<Self, ResolveErrorKind> {
        // SAFETY: the registration at `Root::INDEX` provides `T`.
        unsafe { container.get_transient_unchecked::<T>(Root::INDEX) }.map(InjectTransient)
    }
}

impl<Root, R: DependencyResolver> ResolveLinkedDependency<Root, ByResolver> for R {
    #[inline]
    fn resolve(_root: &Root, container: &Container) -> Result<Self, ResolveErrorKind> {
        R::resolve(container).map_err(Into::into)
    }
}

pub trait ResolveLinkedDependencies<Root, Links>: Sized {
    fn resolve(root: &Root, container: &Container) -> Result<Self, ResolveErrorKind>;
}

macro_rules! impl_resolve_linked_dependencies {
    ([$($dep:ident $index:ident),*]) => {
        impl<Root, $($dep: ResolveLinkedDependency<Root, $index>, $index,)*> ResolveLinkedDependencies<Root, ($($index,)*)> for ($($dep,)*) {
            #[inline]
            #[allow(unused_variables)]
            fn resolve(root: &Root, container: &Container) -> Result<Self, ResolveErrorKind> {
                Ok(($($dep::resolve(root, container)?,)*))
            }
        }
    };
}

all_the_tuple_pairs!(impl_resolve_linked_dependencies);

/// Bridges typed registrations to `RegistrationId`-indexed execution.
///
/// # Safety invariants
///
/// - The boxed tree and owned runtime fragments stay live at stable addresses until all
///   executors and borrowing futures are gone. `Plan` drops executor tables before the tree.
/// - Registration pointers and erased functions remain paired with their exact concrete
///   types; construction receives the matching `Root` pointer and registration ID.
/// - IDs consistently index graph nodes, executors, type metadata and cache slots. Redirects
///   select the entire target executor; graph transformations must preserve this correspondence.
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
    pub(crate) construct:
        unsafe fn(root: *const (), item: *const (), &Container, index: usize) -> Result<RcAnyThreadSafety, ResolveErrorKind>,
    pub(crate) construct_transient:
        unsafe fn(root: *const (), item: *const (), &Container, index: usize, out: *mut ()) -> Result<(), ResolveErrorKind>,
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

impl<Out: 'static, Inst, Deps, Fin: MaybeFinalizer<Out>, Links> Finalize for Linked<Out, Inst, Deps, Fin, Links> {
    unsafe fn finalize(&self, value: RcAnyThreadSafety) {
        // SAFETY: the caller guarantees `value` came from this registration, which provides `Out`.
        self.finalizer.finalize(unsafe { downcast_unchecked::<Out>(value) });
    }
}

/// # Safety
/// As [`construct_erased`]; `out` must be valid for writing an `Item::Provides`.
unsafe fn transient_erased<Root, Item>(
    root: *const (),
    item: *const (),
    container: &Container,
    index: usize,
    out: *mut (),
) -> Result<(), ResolveErrorKind>
where
    Item: ConstructRegistration<Root>,
{
    // SAFETY: the caller supplies a live `Root` and its matching `Item` from the same boxed tree.
    let (root, item) = unsafe { (&*root.cast::<Root>(), &*item.cast::<Item>()) };
    let value = item.construct(root, container, index)?;
    // SAFETY: `out` is aligned, writable uninitialized storage for `Item::Provides`.
    unsafe { out.cast::<Item::Provides>().write(value) };
    Ok(())
}

impl RegistrationExecutor {
    pub(crate) fn of<Root, Item>(item: &Item) -> Self
    where
        Item: ConstructRegistration<Root> + Finalize,
        Item::Provides: SendSafety + SyncSafety,
    {
        Self {
            registration: core::ptr::from_ref(item).cast(),
            construct: construct_erased::<Root, Item>,
            construct_transient: transient_erased::<Root, Item>,
            finalize: finalize_erased::<Item>,
        }
    }
}

/// # Safety
/// `root` must point to a live `Root` and `item` to a live `Item` inside it.
unsafe fn construct_erased<Root, Item>(
    root: *const (),
    item: *const (),
    container: &Container,
    index: usize,
) -> Result<RcAnyThreadSafety, ResolveErrorKind>
where
    Item: ConstructRegistration<Root>,
    Item::Provides: SendSafety + SyncSafety,
{
    // SAFETY: the caller supplies a live `Root` and its matching `Item` from the same boxed tree.
    let (root, item) = unsafe { (&*root.cast::<Root>(), &*item.cast::<Item>()) };
    item.construct_inject(root, container, index)
}

/// Collection order must match graph registration IDs.
pub trait CollectExecutors<Root> {
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn collect_executors(&self, executors: &mut alloc::vec::Vec<RegistrationExecutor>);
}

impl<Root, Out, Inst, Deps, Fin, Links> CollectExecutors<Root> for Linked<Out, Inst, Deps, Fin, Links>
where
    Self: ConstructRegistration<Root, Provides = Out>,
    Out: SendSafety + SyncSafety + 'static,
    Fin: MaybeFinalizer<Out>,
{
    #[allow(private_interfaces)]
    fn collect_executors(&self, executors: &mut alloc::vec::Vec<RegistrationExecutor>) {
        executors.push(RegistrationExecutor::of::<Root, Self>(self));
    }
}

impl<Root, Left: CollectExecutors<Root>, Right: CollectExecutors<Root>> CollectExecutors<Root> for Node<Left, Right> {
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

impl Size for ContainerLeaf {
    const SIZE: usize = 1;
}

impl ProviderPath<Container, Here> for ContainerLeaf {}

impl RegistrationPath<Here> for ContainerLeaf {
    type Registration = Self;
    const INDEX: usize = 0;
}

impl<Root> Link<Root, ()> for ContainerLeaf {
    type Linked = Self;

    #[inline]
    fn link(self) -> Self {
        self
    }
}

impl<Root> ConstructRegistration<Root> for ContainerLeaf {
    type Provides = Container;

    #[inline]
    fn construct(&self, _root: &Root, container: &Container, _index: usize) -> Result<Container, ResolveErrorKind> {
        Ok(container.clone())
    }
}

impl Finalize for ContainerLeaf {
    unsafe fn finalize(&self, _value: RcAnyThreadSafety) {}
}

impl<Root> CollectExecutors<Root> for ContainerLeaf {
    #[allow(private_interfaces)]
    fn collect_executors(&self, executors: &mut Vec<RegistrationExecutor>) {
        executors.push(RegistrationExecutor::of::<Root, Self>(self));
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
impl<Out, Inst, Deps, Fin, Links> CollectRuntime for Linked<Out, Inst, Deps, Fin, Links> {}
impl CollectRuntime for Empty {}
impl CollectRuntime for ContainerLeaf {}

impl<Left: CollectRuntime, Right: CollectRuntime> CollectRuntime for Node<Left, Right> {
    fn collect_runtime<'a>(&'a self, out: &mut Vec<&'a dyn crate::runtime_registry::RuntimeTree>) {
        self.0.collect_runtime(out);
        self.1.collect_runtime(out);
    }
}

pub trait LinkedDependencyMetadata<Root, Path> {
    fn request() -> DependencyRequest<TypeId>;
}

impl<Root: RegistrationPath<Path>, T: 'static, Path> LinkedDependencyMetadata<Root, LinkedInject<Path>> for Inject<T> {
    fn request() -> DependencyRequest<TypeId> {
        DependencyRequest {
            target: Target::Id(RegistrationId(u32::try_from(Root::INDEX).expect("too many registrations"))),
            mode: RequestMode::Inject,
            type_name: core::any::type_name::<T>(),
        }
    }
}

impl<Root: RegistrationPath<Path>, T: 'static, Path> LinkedDependencyMetadata<Root, LinkedInjectTransient<Path>> for InjectTransient<T> {
    fn request() -> DependencyRequest<TypeId> {
        DependencyRequest {
            target: Target::Id(RegistrationId(u32::try_from(Root::INDEX).expect("too many registrations"))),
            mode: RequestMode::InjectTransient,
            type_name: core::any::type_name::<T>(),
        }
    }
}

impl<Root, R: DependencyResolver + 'static> LinkedDependencyMetadata<Root, ByResolver> for R {
    fn request() -> DependencyRequest<TypeId> {
        <R as DependencyMetadata>::request()
    }
}

pub trait LinkedDependenciesMetadata<Root, Links> {
    fn requests() -> Vec<DependencyRequest<TypeId>>;
}

macro_rules! impl_linked_dependencies_metadata {
    ([$($dep:ident $index:ident),*]) => {
        impl<Root, $($dep: LinkedDependencyMetadata<Root, $index>, $index,)*> LinkedDependenciesMetadata<Root, ($($index,)*)> for ($($dep,)*) {
            fn requests() -> Vec<DependencyRequest<TypeId>> {
                alloc::vec![$(<$dep as LinkedDependencyMetadata<Root, $index>>::request()),*]
            }
        }
    };
}

all_the_tuple_pairs!(impl_linked_dependencies_metadata);

/// Static edges enter the graph compiler as `Target::Id`.
pub trait DescribeLinked<Root> {
    fn describe_linked(&self, out: &mut Vec<Registration<TypeId>>);
}

impl<Root, Out: 'static, Inst, Deps, Fin: MaybeFinalizer<Out>, Links> DescribeLinked<Root> for Linked<Out, Inst, Deps, Fin, Links>
where
    Deps: LinkedDependenciesMetadata<Root, Links>,
{
    fn describe_linked(&self, out: &mut Vec<Registration<TypeId>>) {
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

impl<Root, Left: DescribeLinked<Root>, Right: DescribeLinked<Root>> DescribeLinked<Root> for Node<Left, Right> {
    fn describe_linked(&self, out: &mut Vec<Registration<TypeId>>) {
        self.0.describe_linked(out);
        self.1.describe_linked(out);
    }
}

macro_rules! describe_linked_as_describe {
    ($($ty:ty $(, $param:ident)*;)*) => {
        $(impl<Root $(, $param: 'static)*> DescribeLinked<Root> for $ty {
            fn describe_linked(&self, out: &mut Vec<Registration<TypeId>>) {
                self.describe(out);
            }
        })*
    };
}

describe_linked_as_describe! {
    Empty;
    ContainerLeaf;
    crate::runtime_registry::RuntimeNode;
    crate::boundary::ImportLeaf<T>, T;
    crate::boundary::ContextLeaf<T>, T;
}
