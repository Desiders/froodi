//! The typed registry graph.
//!
//! `registry!` builds a balanced binary tree of registrations: [`Reg`] leaves under [`Node`]s.
//! The tree is an ordinary value, so factory values, including captured closures, are stored in
//! it as they are. Its type records, for every registration, the provided type, the factory type
//! and the dependency tuple, so rustc can answer structural questions about the registry:
//!
//! - [`Has<T, I>`]: which leaf provides `T`. The path `I` is inferred; no leaf is a
//!   "missing binding" error, two leaves are an ambiguity error.
//! - [`At<I>`]: the leaf at a known path, with its position as a constant.
//!
//! [`Link`] runs `Has` for every dependency of every registration against the whole tree and
//! records the found paths in the leaf type ([`Linked`]). After linking, executing a dependency
//! edge is a path lookup (`At`) that rustc resolves at compile time; no `TypeId` is involved.
//!
//! The tree is balanced so trait resolution depth grows with `log2(n)`, not `n`.

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

/// Path step: the leaf itself.
pub struct Here;
/// Path step: into the left subtree.
pub struct L<I>(PhantomData<I>);
/// Path step: into the right subtree.
pub struct R<I>(PhantomData<I>);

/// A registration before linking.
pub struct Reg<T, F, D, Fin> {
    pub(crate) factory: F,
    pub(crate) finalizer: Fin,
    pub(crate) meta: Meta,
    marker: PhantomData<fn() -> (T, D)>,
}

/// What `registry!` knows about a registration besides its factory.
#[derive(Clone, Copy)]
pub struct Meta {
    pub(crate) scope: ScopeData,
    pub(crate) source: ValueSource,
    pub(crate) origin: Origin,
    pub(crate) config: Config,
}

impl<T, F, D, Fin> Reg<T, F, D, Fin> {
    #[inline]
    pub(crate) const fn new(factory: F, finalizer: Fin, meta: Meta) -> Self {
        Self {
            factory,
            finalizer,
            meta,
            marker: PhantomData,
        }
    }
}

/// A tree without registrations: `registry!()`.
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

impl<Root> Walk<Root> for Empty {
    #[allow(private_interfaces)]
    fn walk(&self, _entries: &mut Vec<Entry>) {}
}

/// An inner node of the registration tree.
pub struct Node<A, B>(pub A, pub B);

/// Number of leaves, used to turn paths into positions.
pub trait Size {
    const SIZE: usize;
}

/// Dependency requests of one factory parameter, for the registration IR.
pub trait DepMeta {
    fn request() -> DependencyRequest<TypeId>;
}

impl<T: 'static> DepMeta for Inject<T> {
    fn request() -> DependencyRequest<TypeId> {
        DependencyRequest {
            target: Target::Key(TypeId::of::<T>()),
            mode: RequestMode::Shared,
            type_name: core::any::type_name::<T>(),
        }
    }
}

impl<T: 'static> DepMeta for InjectTransient<T> {
    fn request() -> DependencyRequest<TypeId> {
        DependencyRequest {
            target: Target::Key(TypeId::of::<T>()),
            mode: RequestMode::Transient,
            type_name: core::any::type_name::<T>(),
        }
    }
}

impl<R: DependencyResolver + 'static> DepMeta for R {
    fn request() -> DependencyRequest<TypeId> {
        DependencyRequest {
            target: Target::Key(TypeId::of::<R>()),
            mode: RequestMode::Resolver,
            type_name: core::any::type_name::<R>(),
        }
    }
}

/// Dependency requests of all factory parameters.
pub trait DepsMeta {
    fn requests() -> Vec<DependencyRequest<TypeId>>;
}

macro_rules! impl_deps_meta {
    ([$($dep:ident),*]) => {
        impl<$($dep: DepMeta,)*> DepsMeta for ($($dep,)*) {
            fn requests() -> Vec<DependencyRequest<TypeId>> {
                alloc::vec![$($dep::request()),*]
            }
        }
    };
}

all_the_tuples!(impl_deps_meta);

/// Describes the registrations of a tree in the registration IR, in declaration order.
pub trait Describe {
    fn describe(&self, out: &mut Vec<Registration<TypeId>>);
}

impl<T: 'static, F, D: DepsMeta, Fin: MaybeFinalizer<T>> Describe for Reg<T, F, D, Fin> {
    fn describe(&self, out: &mut Vec<Registration<TypeId>>) {
        out.push(Registration {
            key: TypeId::of::<T>(),
            type_name: core::any::type_name::<T>(),
            requests: D::requests(),
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

impl<A: Describe, B: Describe> Describe for Node<A, B> {
    fn describe(&self, out: &mut Vec<Registration<TypeId>>) {
        self.0.describe(out);
        self.1.describe(out);
    }
}

impl<T, F, D, Fin> Size for Reg<T, F, D, Fin> {
    const SIZE: usize = 1;
}

impl<A: Size, B: Size> Size for Node<A, B> {
    const SIZE: usize = A::SIZE + B::SIZE;
}

/// Finds the registration that provides `T`. `I` is the path to it and is inferred by rustc.
#[diagnostic::on_unimplemented(
    message = "no registration provides `{T}`",
    label = "`{T}` is requested here, but no `provide(...)` in the registry produces it",
    note = "register a factory or `instance(...)` that returns `{T}`, or `extend(...)` a registry that does"
)]
pub trait Has<T, I> {}

impl<T, F, D, Fin> Has<T, Here> for Reg<T, F, D, Fin> {}

impl<T, I, A: Has<T, I>, B> Has<T, L<I>> for Node<A, B> {}

impl<T, I, A, B: Has<T, I>> Has<T, R<I>> for Node<A, B> {}

/// The leaf at path `I`.
pub trait At<I> {
    type Item;
    /// Position of the leaf in declaration order.
    const INDEX: usize;

    fn at(&self) -> &Self::Item;
}

impl<T, F, D, Fin, DI> At<Here> for Linked<T, F, D, Fin, DI> {
    type Item = Self;
    const INDEX: usize = 0;

    #[inline]
    fn at(&self) -> &Self {
        self
    }
}

impl<I, A: At<I>, B> At<L<I>> for Node<A, B> {
    type Item = A::Item;
    const INDEX: usize = A::INDEX;

    #[inline]
    fn at(&self) -> &Self::Item {
        self.0.at()
    }
}

impl<I, A: Size, B: At<I>> At<R<I>> for Node<A, B> {
    type Item = B::Item;
    const INDEX: usize = A::SIZE + B::INDEX;

    #[inline]
    fn at(&self) -> &Self::Item {
        self.1.at()
    }
}

/// Index of an `Inject<T>` dependency: the path to its provider.
pub struct SharedAt<I>(PhantomData<I>);

/// A factory parameter that can be linked against the registry tree `Root`.
#[diagnostic::on_unimplemented(
    message = "no registration provides the factory parameter `{Self}`",
    label = "this registry has no `provide(...)` for the type inside `{Self}`",
    note = "register a factory or `instance(...)` for it, or `extend(...)` a registry that does"
)]
pub trait DepIn<Root, I> {}

impl<Root: Has<T, I>, T, I> DepIn<Root, SharedAt<I>> for Inject<T> {}

/// Index of an `InjectTransient<T>` dependency: the path to its provider.
pub struct TransientAt<I>(PhantomData<I>);

impl<Root: Has<T, I>, T, I> DepIn<Root, TransientAt<I>> for InjectTransient<T> {}

/// Index of a custom resolver parameter: there is nothing to link.
pub struct ByResolver;

impl<Root, R: DependencyResolver> DepIn<Root, ByResolver> for R {}

/// All parameters of a factory, linked against `Root`.
pub trait DepsIn<Root, I> {}

macro_rules! impl_deps_in {
    ([$($dep:ident $index:ident),*]) => {
        impl<Root, $($dep: DepIn<Root, $index>, $index,)*> DepsIn<Root, ($($index,)*)> for ($($dep,)*) {}
    };
}

all_the_tuple_pairs!(impl_deps_in);

type LinkedMarker<T, D, DI> = PhantomData<fn() -> (T, D, DI)>;

/// A registration whose dependency paths `DI` are resolved.
pub struct Linked<T, F, D, Fin, DI> {
    pub(crate) factory: F,
    pub(crate) finalizer: Fin,
    pub(crate) meta: Meta,
    marker: LinkedMarker<T, D, DI>,
}

impl<T, F, D, Fin, DI> Size for Linked<T, F, D, Fin, DI> {
    const SIZE: usize = 1;
}

/// Resolves every dependency path of the tree against `Root`, producing the linked tree.
/// `Links` mirrors the tree shape and is inferred by rustc.
pub trait Link<Root, Links> {
    type Linked;

    fn link(self) -> Self::Linked;
}

impl<Root, T, F, D, Fin, DI> Link<Root, DI> for Reg<T, F, D, Fin>
where
    D: DepsIn<Root, DI>,
{
    type Linked = Linked<T, F, D, Fin, DI>;

    #[inline]
    fn link(self) -> Self::Linked {
        Linked {
            factory: self.factory,
            finalizer: self.finalizer,
            meta: self.meta,
            marker: PhantomData,
        }
    }
}

impl<Root, A: Link<Root, LA>, B: Link<Root, LB>, LA, LB> Link<Root, (LA, LB)> for Node<A, B> {
    type Linked = Node<A::Linked, B::Linked>;

    #[inline]
    fn link(self) -> Self::Linked {
        Node(self.0.link(), self.1.link())
    }
}

/// Constructs the value of a linked registration inside the linked tree `Root`.
#[diagnostic::on_unimplemented(
    message = "this registration cannot be constructed synchronously",
    label = "a sync factory depends on it",
    note = "a sync factory may not depend on an async registration; make the dependent factory async"
)]
pub trait Exec<Root> {
    type Provides: 'static;

    /// # Errors
    /// Returns the error of a dependency or of the factory.
    fn construct(&self, root: &Root, container: &Container, index: usize) -> Result<Self::Provides, ResolveErrorKind>;

    /// The shared form of the value for `get` semantics. A registration that stands for another
    /// one returns that registration's value instead of a new allocation.
    ///
    /// # Errors
    /// Returns the error of a dependency or of the factory.
    #[inline]
    fn construct_shared(&self, root: &Root, container: &Container, index: usize) -> Result<RcAnyThreadSafety, ResolveErrorKind>
    where
        Self::Provides: SendSafety + SyncSafety,
    {
        Ok(RcThreadSafety::new(self.construct(root, container, index)?) as RcAnyThreadSafety)
    }
}

impl<Root, T: 'static, F, D, Fin, DI> Exec<Root> for Linked<T, F, D, Fin, DI>
where
    F: Instantiator<D, Provides = T>,
    D: DepsExec<Root, DI>,
{
    type Provides = T;

    #[inline]
    fn construct(&self, root: &Root, container: &Container, _index: usize) -> Result<T, ResolveErrorKind> {
        let dependencies =
            D::resolve(root, container).map_err(|err| ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(err.into())))?;
        self.factory
            .clone()
            .instantiate(dependencies)
            .map_err(|err| ResolveErrorKind::Instantiator(InstantiatorErrorKind::Factory(err.into())))
    }
}

/// Resolves one factory parameter at its linked path.
#[diagnostic::on_unimplemented(
    message = "no registration provides the factory parameter `{Self}`",
    label = "this registry has no `provide(...)` for the type inside `{Self}`"
)]
pub trait DepExec<Root, I>: Sized {
    /// # Errors
    /// Returns the error of resolving the dependency.
    fn resolve(root: &Root, container: &Container) -> Result<Self, ResolveErrorKind>;
}

impl<Root, T: SendSafety + SyncSafety + 'static, I> DepExec<Root, SharedAt<I>> for Inject<T>
where
    Root: At<I>,
    Root::Item: Exec<Root, Provides = T>,
{
    #[inline]
    fn resolve(root: &Root, container: &Container) -> Result<Self, ResolveErrorKind> {
        let value = container.shared_with(Root::INDEX, |owner| root.at().construct_shared(root, owner, Root::INDEX))?;
        // SAFETY: slot `Root::INDEX` only ever holds values of the registration at path `I`,
        // which provides `T`.
        Ok(Inject(unsafe { downcast_unchecked(value) }))
    }
}

impl<Root, T: 'static, I> DepExec<Root, TransientAt<I>> for InjectTransient<T>
where
    Root: At<I>,
    Root::Item: Exec<Root, Provides = T>,
{
    #[inline]
    fn resolve(root: &Root, container: &Container) -> Result<Self, ResolveErrorKind> {
        container
            .transient_with(Root::INDEX, |container| root.at().construct(root, container, Root::INDEX))
            .map(InjectTransient)
    }
}

impl<Root, R: DependencyResolver> DepExec<Root, ByResolver> for R {
    #[inline]
    fn resolve(_root: &Root, container: &Container) -> Result<Self, ResolveErrorKind> {
        R::resolve(container).map_err(Into::into)
    }
}

/// Resolves all parameters of a factory.
pub trait DepsExec<Root, I>: Sized {
    /// # Errors
    /// Returns the first dependency error.
    fn resolve(root: &Root, container: &Container) -> Result<Self, ResolveErrorKind>;
}

macro_rules! impl_deps_exec {
    ([$($dep:ident $index:ident),*]) => {
        impl<Root, $($dep: DepExec<Root, $index>, $index,)*> DepsExec<Root, ($($index,)*)> for ($($dep,)*) {
            #[inline]
            #[allow(unused_variables)]
            fn resolve(root: &Root, container: &Container) -> Result<Self, ResolveErrorKind> {
                Ok(($($dep::resolve(root, container)?,)*))
            }
        }
    };
}

all_the_tuple_pairs!(impl_deps_exec);

/// Construction entry of one registration, reachable from the public `get::<T>()` boundary.
pub(crate) struct Entry {
    /// The leaf inside the boxed linked tree.
    pub(crate) item: *const (),
    pub(crate) construct:
        unsafe fn(root: *const (), item: *const (), &Container, index: usize) -> Result<RcAnyThreadSafety, ResolveErrorKind>,
    /// Constructs a fresh value into `out`, which must point to uninitialized memory for the
    /// registration's provided type.
    pub(crate) transient:
        unsafe fn(root: *const (), item: *const (), &Container, index: usize, out: *mut ()) -> Result<(), ResolveErrorKind>,
    /// Runs the registration's finalizer on a value it provided.
    pub(crate) finalize: unsafe fn(item: *const (), value: RcAnyThreadSafety),
}

/// # Safety
/// `item` must point to a live `Item`, and `value` must have been provided by it.
unsafe fn finalize_erased<Item: Finalize>(item: *const (), value: RcAnyThreadSafety) {
    // SAFETY: guaranteed by the caller.
    unsafe { (*item.cast::<Item>()).finalize(value) };
}

/// Runs a linked registration's finalizer, if it has one.
pub trait Finalize {
    /// # Safety
    /// `value` must have been provided by this registration.
    unsafe fn finalize(&self, value: RcAnyThreadSafety);
}

impl<T: 'static, F, D, Fin: MaybeFinalizer<T>, DI> Finalize for Linked<T, F, D, Fin, DI> {
    unsafe fn finalize(&self, value: RcAnyThreadSafety) {
        // SAFETY: the caller guarantees `value` came from this registration, which provides `T`.
        self.finalizer.finalize(unsafe { downcast_unchecked::<T>(value) });
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
    Item: Exec<Root>,
{
    // SAFETY: guaranteed by the caller; both pointers come from the same boxed tree.
    let (root, item) = unsafe { (&*root.cast::<Root>(), &*item.cast::<Item>()) };
    let value = item.construct(root, container, index)?;
    // SAFETY: guaranteed by the caller.
    unsafe { out.cast::<Item::Provides>().write(value) };
    Ok(())
}

impl Entry {
    /// The entry of a static leaf: construction through `Exec`, finalization through `Finalize`.
    pub(crate) fn of<Root, Item>(item: &Item) -> Self
    where
        Item: Exec<Root> + Finalize,
        Item::Provides: SendSafety + SyncSafety,
    {
        Self {
            item: core::ptr::from_ref(item).cast(),
            construct: construct_erased::<Root, Item>,
            transient: transient_erased::<Root, Item>,
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
    Item: Exec<Root>,
    Item::Provides: SendSafety + SyncSafety,
{
    // SAFETY: guaranteed by the caller; both pointers come from the same boxed tree.
    let (root, item) = unsafe { (&*root.cast::<Root>(), &*item.cast::<Item>()) };
    item.construct_shared(root, container, index)
}

/// Collects the construction entries of a linked tree in declaration order.
pub trait Walk<Root> {
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn walk(&self, entries: &mut alloc::vec::Vec<Entry>);
}

impl<Root, T, F, D, Fin, DI> Walk<Root> for Linked<T, F, D, Fin, DI>
where
    Self: Exec<Root, Provides = T>,
    T: SendSafety + SyncSafety + 'static,
    Fin: MaybeFinalizer<T>,
{
    #[allow(private_interfaces)]
    fn walk(&self, entries: &mut alloc::vec::Vec<Entry>) {
        entries.push(Entry::of::<Root, Self>(self));
    }
}

impl<Root, A: Walk<Root>, B: Walk<Root>> Walk<Root> for Node<A, B> {
    #[allow(private_interfaces)]
    fn walk(&self, entries: &mut alloc::vec::Vec<Entry>) {
        self.0.walk(entries);
        self.1.walk(entries);
    }
}

/// The container itself as a registration: root scope, never cached, as in Froodi (caching the
/// container in its own cache would keep it alive). `Container::new` appends it to every tree.
pub struct ContainerLeaf {
    pub(crate) scope: ScopeData,
}

impl Size for ContainerLeaf {
    const SIZE: usize = 1;
}

impl Has<Container, Here> for ContainerLeaf {}

impl At<Here> for ContainerLeaf {
    type Item = Self;
    const INDEX: usize = 0;

    #[inline]
    fn at(&self) -> &Self {
        self
    }
}

impl<Root> Link<Root, ()> for ContainerLeaf {
    type Linked = Self;

    #[inline]
    fn link(self) -> Self {
        self
    }
}

impl<Root> Exec<Root> for ContainerLeaf {
    type Provides = Container;

    #[inline]
    fn construct(&self, _root: &Root, container: &Container, _index: usize) -> Result<Container, ResolveErrorKind> {
        Ok(container.clone())
    }
}

impl Finalize for ContainerLeaf {
    unsafe fn finalize(&self, _value: RcAnyThreadSafety) {}
}

impl<Root> Walk<Root> for ContainerLeaf {
    #[allow(private_interfaces)]
    fn walk(&self, entries: &mut Vec<Entry>) {
        entries.push(Entry::of::<Root, Self>(self));
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

/// Collects the runtime registries nested in a tree, in tree order. Their registrations are
/// numbered after every static one.
pub trait CollectRuntime {
    #[doc(hidden)]
    fn collect_runtime<'a>(&'a self, _out: &mut Vec<&'a dyn crate::runtime_registry::RuntimeTree>) {}
}

impl<T, F, D, Fin> CollectRuntime for Reg<T, F, D, Fin> {}
impl<T, F, D, Fin, DI> CollectRuntime for Linked<T, F, D, Fin, DI> {}
impl CollectRuntime for Empty {}
impl CollectRuntime for ContainerLeaf {}

impl<A: CollectRuntime, B: CollectRuntime> CollectRuntime for Node<A, B> {
    fn collect_runtime<'a>(&'a self, out: &mut Vec<&'a dyn crate::runtime_registry::RuntimeTree>) {
        self.0.collect_runtime(out);
        self.1.collect_runtime(out);
    }
}

/// The IR request of a linked factory parameter: static edges carry the id rustc resolved.
pub trait DepExecMeta<Root, I> {
    fn request() -> DependencyRequest<TypeId>;
}

impl<Root: At<I>, T: 'static, I> DepExecMeta<Root, SharedAt<I>> for Inject<T> {
    fn request() -> DependencyRequest<TypeId> {
        DependencyRequest {
            target: Target::Id(RegistrationId(u32::try_from(Root::INDEX).expect("too many registrations"))),
            mode: RequestMode::Shared,
            type_name: core::any::type_name::<T>(),
        }
    }
}

impl<Root: At<I>, T: 'static, I> DepExecMeta<Root, TransientAt<I>> for InjectTransient<T> {
    fn request() -> DependencyRequest<TypeId> {
        DependencyRequest {
            target: Target::Id(RegistrationId(u32::try_from(Root::INDEX).expect("too many registrations"))),
            mode: RequestMode::Transient,
            type_name: core::any::type_name::<T>(),
        }
    }
}

impl<Root, R: DependencyResolver + 'static> DepExecMeta<Root, ByResolver> for R {
    fn request() -> DependencyRequest<TypeId> {
        <R as DepMeta>::request()
    }
}

/// IR requests of all parameters of a linked factory.
pub trait DepsExecMeta<Root, I> {
    fn requests() -> Vec<DependencyRequest<TypeId>>;
}

macro_rules! impl_deps_exec_meta {
    ([$($dep:ident $index:ident),*]) => {
        impl<Root, $($dep: DepExecMeta<Root, $index>, $index,)*> DepsExecMeta<Root, ($($index,)*)> for ($($dep,)*) {
            fn requests() -> Vec<DependencyRequest<TypeId>> {
                alloc::vec![$(<$dep as DepExecMeta<Root, $index>>::request()),*]
            }
        }
    };
}

all_the_tuple_pairs!(impl_deps_exec_meta);

/// Describes a linked tree in the IR. Static edges enter the graph compiler as `Target::Id`.
pub trait DescribeLinked<Root> {
    fn describe_linked(&self, out: &mut Vec<Registration<TypeId>>);
}

impl<Root, T: 'static, F, D, Fin: MaybeFinalizer<T>, DI> DescribeLinked<Root> for Linked<T, F, D, Fin, DI>
where
    D: DepsExecMeta<Root, DI>,
{
    fn describe_linked(&self, out: &mut Vec<Registration<TypeId>>) {
        out.push(Registration {
            key: TypeId::of::<T>(),
            type_name: core::any::type_name::<T>(),
            requests: D::requests(),
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

impl<Root, A: DescribeLinked<Root>, B: DescribeLinked<Root>> DescribeLinked<Root> for Node<A, B> {
    fn describe_linked(&self, out: &mut Vec<Registration<TypeId>>) {
        self.0.describe_linked(out);
        self.1.describe_linked(out);
    }
}

/// Leaves without factory parameters describe themselves the same way linked or not.
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
