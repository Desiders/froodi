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

use core::marker::PhantomData;

use crate::{
    container::Container,
    errors::{InstantiatorErrorKind, ResolveErrorKind},
    inject::Inject,
    instantiator::Instantiator,
    thread_safety::{RcAnyThreadSafety, RcThreadSafety, SendSafety, SyncSafety},
};

/// Path step: the leaf itself.
pub struct Here;
/// Path step: into the left subtree.
pub struct L<I>(PhantomData<I>);
/// Path step: into the right subtree.
pub struct R<I>(PhantomData<I>);

/// A registration before linking.
pub struct Reg<T, F, D> {
    pub(crate) factory: F,
    marker: PhantomData<fn() -> (T, D)>,
}

impl<T, F, D> Reg<T, F, D> {
    #[inline]
    pub(crate) const fn new(factory: F) -> Self {
        Self {
            factory,
            marker: PhantomData,
        }
    }
}

/// An inner node of the registration tree.
pub struct Node<A, B>(pub A, pub B);

/// Number of leaves, used to turn paths into positions.
pub trait Size {
    const SIZE: usize;
}

impl<T, F, D> Size for Reg<T, F, D> {
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

impl<T, F, D> Has<T, Here> for Reg<T, F, D> {}

impl<T, I, A: Has<T, I>, B> Has<T, L<I>> for Node<A, B> {}

impl<T, I, A, B: Has<T, I>> Has<T, R<I>> for Node<A, B> {}

/// The leaf at path `I`.
pub trait At<I> {
    type Item;
    /// Position of the leaf in declaration order.
    const INDEX: usize;

    fn at(&self) -> &Self::Item;
}

impl<T, F, D, DI> At<Here> for Linked<T, F, D, DI> {
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
pub trait DepIn<Root, I> {}

impl<Root: Has<T, I>, T, I> DepIn<Root, SharedAt<I>> for Inject<T> {}

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
pub struct Linked<T, F, D, DI> {
    pub(crate) factory: F,
    marker: LinkedMarker<T, D, DI>,
}

impl<T, F, D, DI> Size for Linked<T, F, D, DI> {
    const SIZE: usize = 1;
}

/// Resolves every dependency path of the tree against `Root`, producing the linked tree.
/// `Links` mirrors the tree shape and is inferred by rustc.
pub trait Link<Root, Links> {
    type Linked;

    fn link(self) -> Self::Linked;
}

impl<Root, T, F, D, DI> Link<Root, DI> for Reg<T, F, D>
where
    D: DepsIn<Root, DI>,
{
    type Linked = Linked<T, F, D, DI>;

    #[inline]
    fn link(self) -> Self::Linked {
        Linked {
            factory: self.factory,
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
pub trait Exec<Root> {
    type Provides: 'static;

    /// # Errors
    /// Returns the error of a dependency or of the factory.
    fn construct(&self, root: &Root, container: &Container) -> Result<Self::Provides, ResolveErrorKind>;
}

impl<Root, T: 'static, F, D, DI> Exec<Root> for Linked<T, F, D, DI>
where
    F: Instantiator<D, Provides = T>,
    D: DepsExec<Root, DI>,
{
    type Provides = T;

    #[inline]
    fn construct(&self, root: &Root, container: &Container) -> Result<T, ResolveErrorKind> {
        let dependencies =
            D::resolve(root, container).map_err(|err| ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(err.into())))?;
        self.factory
            .clone()
            .instantiate(dependencies)
            .map_err(|err| ResolveErrorKind::Instantiator(InstantiatorErrorKind::Factory(err.into())))
    }
}

/// Resolves one factory parameter at its linked path.
pub trait DepExec<Root, I>: Sized {
    /// # Errors
    /// Returns the error of resolving the dependency.
    fn resolve(root: &Root, container: &Container) -> Result<Self, ResolveErrorKind>;
}

impl<Root, T, I> DepExec<Root, SharedAt<I>> for Inject<T>
where
    Root: At<I>,
    Root::Item: Exec<Root, Provides = T>,
{
    #[inline]
    fn resolve(root: &Root, container: &Container) -> Result<Self, ResolveErrorKind> {
        Ok(Inject(RcThreadSafety::new(root.at().construct(root, container)?)))
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
    pub(crate) type_id: core::any::TypeId,
    /// The leaf inside the boxed linked tree.
    pub(crate) item: *const (),
    pub(crate) construct: unsafe fn(root: *const (), item: *const (), &Container) -> Result<RcAnyThreadSafety, ResolveErrorKind>,
}

/// # Safety
/// `root` must point to a live `Root` and `item` to a live `Item` inside it.
unsafe fn construct_erased<Root, Item>(
    root: *const (),
    item: *const (),
    container: &Container,
) -> Result<RcAnyThreadSafety, ResolveErrorKind>
where
    Item: Exec<Root>,
    Item::Provides: SendSafety + SyncSafety,
{
    // SAFETY: guaranteed by the caller; both pointers come from the same boxed tree.
    let (root, item) = unsafe { (&*root.cast::<Root>(), &*item.cast::<Item>()) };
    Ok(RcThreadSafety::new(item.construct(root, container)?) as RcAnyThreadSafety)
}

/// Collects the construction entries of a linked tree in declaration order.
pub trait Walk<Root> {
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn walk(&self, entries: &mut alloc::vec::Vec<Entry>);
}

impl<Root, T, F, D, DI> Walk<Root> for Linked<T, F, D, DI>
where
    Self: Exec<Root, Provides = T>,
    T: SendSafety + SyncSafety + 'static,
{
    #[allow(private_interfaces)]
    fn walk(&self, entries: &mut alloc::vec::Vec<Entry>) {
        entries.push(Entry {
            type_id: core::any::TypeId::of::<T>(),
            item: core::ptr::from_ref(self).cast(),
            construct: construct_erased::<Root, Self>,
        });
    }
}

impl<Root, A: Walk<Root>, B: Walk<Root>> Walk<Root> for Node<A, B> {
    #[allow(private_interfaces)]
    fn walk(&self, entries: &mut alloc::vec::Vec<Entry>) {
        self.0.walk(entries);
        self.1.walk(entries);
    }
}
