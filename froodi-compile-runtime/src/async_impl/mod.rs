//! Async execution on the same registry tree, IR and graph compiler as the sync engine.
//!
//! `async_registry!` builds the same tree as `registry!`, with async factory leaves
//! ([`AsyncReg`]). Sync registrations join it through `extend(...)`. The async [`Container`]
//! wraps the sync one: scopes, caches, context and the finalizer list are shared; only
//! construction is async.
//!
//! On a static edge an async factory awaits its dependency's construction directly; a sync
//! dependency is called directly. A future is boxed only where a call leaves the typed world:
//! at the public `get` boundary and when a value is resolved in an ancestor container. A sync
//! factory cannot depend on an async registration: that is a compile error, because the sync
//! call cannot await.

#![allow(
    clippy::manual_async_fn,
    reason = "trait signatures state the `SendSafety` bound on returned futures"
)]

use alloc::{boxed::Box, vec::Vec};
use core::{any::TypeId, future::Future, marker::PhantomData, pin::Pin};

use froodi_compile_core::{Diagnostics, ExecutionKind, Registration};

use crate::{
    config::Config,
    container::{self, Linked, Plan},
    dependency_resolver::DependencyResolver,
    errors::{InstantiateErrorKind, InstantiatorErrorKind, ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind, TypeInfo},
    finalizer::{NoFinalizer, WithFinalizer},
    graph::{
        At, ByResolver, CollectRuntime, ContainerLeaf, DepsExecMeta, DepsIn, DepsMeta, Describe, DescribeLinked, Entry, Exec, Finalize,
        Has, Here, Link, Meta, Node, SharedAt, Size, TransientAt, Walk,
    },
    inject::{Inject, InjectTransient},
    registry::Registry,
    scope::Scope,
    thread_safety::{downcast_unchecked, BoxAnyThreadSafety, RcAnyThreadSafety, RcThreadSafety, SendSafety, SyncSafety},
};

#[cfg(feature = "thread_safe")]
type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
#[cfg(not(feature = "thread_safe"))]
type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// An async factory: a function or closure returning a future of its result. Same shape as
/// Froodi's async `Instantiator<Deps>`.
pub trait Instantiator<Deps>: Clone + 'static {
    type Provides: 'static;
    type Error: Into<InstantiateErrorKind>;

    fn instantiate(&mut self, dependencies: Deps) -> impl Future<Output = Result<Self::Provides, Self::Error>> + SendSafety;
}

macro_rules! impl_async_instantiator {
    ([$($ty:ident),*]) => {
        #[allow(non_snake_case)]
        impl<F, Fut, Response, Err, $($ty,)*> Instantiator<($($ty,)*)> for F
        where
            F: FnMut($($ty,)*) -> Fut + Clone + 'static,
            Fut: Future<Output = Result<Response, Err>> + SendSafety,
            Response: 'static,
            Err: Into<InstantiateErrorKind>,
        {
            type Provides = Response;
            type Error = Err;

            #[inline]
            fn instantiate(&mut self, ($($ty,)*): ($($ty,)*)) -> impl Future<Output = Result<Response, Err>> + SendSafety {
                self($($ty,)*)
            }
        }
    };
}

all_the_tuples!(impl_async_instantiator);

/// An async finalizer, as in Froodi.
pub trait Finalizer<Dep>: Clone + 'static {
    fn finalize(&mut self, dependency: RcThreadSafety<Dep>) -> impl Future<Output = ()> + SendSafety;
}

impl<F, Fut, Dep> Finalizer<Dep> for F
where
    F: FnMut(RcThreadSafety<Dep>) -> Fut + Clone + 'static,
    Fut: Future<Output = ()> + SendSafety,
{
    #[inline]
    fn finalize(&mut self, dependency: RcThreadSafety<Dep>) -> impl Future<Output = ()> + SendSafety {
        self(dependency)
    }
}

/// The finalizer slot of an async registration. `finalize` consumes a clone of the slot, as
/// Froodi calls a clone of the finalizer, so the returned future owns everything it uses.
pub trait MaybeFinalizer<Dep>: Clone {
    const PRESENT: bool;

    fn finalize(self, dependency: RcThreadSafety<Dep>) -> impl Future<Output = ()> + SendSafety;
}

impl<Dep: SendSafety + SyncSafety> MaybeFinalizer<Dep> for NoFinalizer {
    const PRESENT: bool = false;

    #[inline]
    fn finalize(self, _dependency: RcThreadSafety<Dep>) -> impl Future<Output = ()> + SendSafety {
        core::future::ready(())
    }
}

impl<Dep: SendSafety + SyncSafety, F: Finalizer<Dep> + SendSafety> MaybeFinalizer<Dep> for WithFinalizer<F> {
    const PRESENT: bool = true;

    #[inline]
    fn finalize(self, dependency: RcThreadSafety<Dep>) -> impl Future<Output = ()> + SendSafety {
        let mut finalizer = self.0;
        async move { finalizer.finalize(dependency).await }
    }
}

/// An async registration before linking.
pub struct AsyncReg<T, F, D, Fin> {
    factory: F,
    finalizer: Fin,
    meta: Meta,
    marker: PhantomData<fn() -> (T, D)>,
}

/// One `provide(...)` item of `async_registry!`.
#[doc(hidden)]
#[inline]
pub fn async_reg<S: Scope, F, D, Fin>(
    scope: S,
    factory: F,
    config: Option<Config>,
    finalizer: Fin,
    source: froodi_compile_core::ValueSource,
    origin: froodi_compile_core::Origin,
) -> AsyncReg<F::Provides, F, D, Fin>
where
    F: Instantiator<D>,
    Fin: MaybeFinalizer<F::Provides>,
{
    AsyncReg {
        factory,
        finalizer,
        meta: Meta {
            scope: scope.into(),
            source,
            origin,
            config: config.unwrap_or_default(),
        },
        marker: PhantomData,
    }
}

fn registration<T: 'static>(
    meta: &Meta,
    finalizer: bool,
    requests: Vec<froodi_compile_core::DependencyRequest<TypeId>>,
) -> Registration<TypeId> {
    Registration {
        key: TypeId::of::<T>(),
        type_name: core::any::type_name::<T>(),
        requests,
        scope: meta.scope.into(),
        cache_provides: meta.config.cache_provides,
        finalizer: finalizer.then_some(ExecutionKind::Async),
        execution: ExecutionKind::Async,
        source: meta.source,
        replaces: false,
        origin: Some(meta.origin),
    }
}

impl<T, F, D, Fin> Size for AsyncReg<T, F, D, Fin> {
    const SIZE: usize = 1;
}

impl<T, F, D, Fin> Has<T, Here> for AsyncReg<T, F, D, Fin> {}

impl<T, F, D, Fin> CollectRuntime for AsyncReg<T, F, D, Fin> {}

impl<T: 'static, F, D: DepsMeta, Fin: MaybeFinalizer<T>> Describe for AsyncReg<T, F, D, Fin> {
    fn describe(&self, out: &mut Vec<Registration<TypeId>>) {
        out.push(registration::<T>(&self.meta, Fin::PRESENT, D::requests()));
    }
}

impl<Root, T, F, D: DepsIn<Root, DI>, Fin, DI> Link<Root, DI> for AsyncReg<T, F, D, Fin> {
    type Linked = AsyncLinked<T, F, D, Fin, DI>;

    #[inline]
    fn link(self) -> Self::Linked {
        AsyncLinked {
            factory: self.factory,
            finalizer: self.finalizer,
            meta: self.meta,
            marker: PhantomData,
        }
    }
}

type AsyncLinkedMarker<T, D, DI> = PhantomData<fn() -> (T, D, DI)>;

/// An async registration whose dependency paths `DI` are resolved.
pub struct AsyncLinked<T, F, D, Fin, DI> {
    factory: F,
    finalizer: Fin,
    meta: Meta,
    marker: AsyncLinkedMarker<T, D, DI>,
}

impl<T, F, D, Fin, DI> Size for AsyncLinked<T, F, D, Fin, DI> {
    const SIZE: usize = 1;
}

impl<T, F, D, Fin, DI> At<Here> for AsyncLinked<T, F, D, Fin, DI> {
    type Item = Self;
    const INDEX: usize = 0;

    #[inline]
    fn at(&self) -> &Self {
        self
    }
}

impl<T, F, D, Fin, DI> CollectRuntime for AsyncLinked<T, F, D, Fin, DI> {}

impl<Root, T: 'static, F, D: DepsExecMeta<Root, DI>, Fin: MaybeFinalizer<T>, DI> DescribeLinked<Root> for AsyncLinked<T, F, D, Fin, DI> {
    fn describe_linked(&self, out: &mut Vec<Registration<TypeId>>) {
        out.push(registration::<T>(&self.meta, Fin::PRESENT, D::requests()));
    }
}

impl<T, F, D, Fin, DI> Finalize for AsyncLinked<T, F, D, Fin, DI> {
    /// Async finalizers run only from the async container's `close().await`.
    unsafe fn finalize(&self, _value: RcAnyThreadSafety) {}
}

/// # Safety
/// Never dereferences its pointers.
unsafe fn async_only<T: 'static>(
    _root: *const (),
    _item: *const (),
    _container: &container::Container,
    _index: usize,
) -> Result<RcAnyThreadSafety, ResolveErrorKind> {
    Err(ResolveErrorKind::AsyncOnly {
        type_info: TypeInfo::of::<T>(),
    })
}

/// # Safety
/// Never dereferences its pointers.
unsafe fn async_only_transient<T: 'static>(
    _root: *const (),
    _item: *const (),
    _container: &container::Container,
    _index: usize,
    _out: *mut (),
) -> Result<(), ResolveErrorKind> {
    Err(ResolveErrorKind::AsyncOnly {
        type_info: TypeInfo::of::<T>(),
    })
}

/// # Safety
/// Never dereferences its pointers.
unsafe fn no_finalize(_item: *const (), _value: RcAnyThreadSafety) {}

impl<Root, T: 'static, F, D, Fin, DI> Walk<Root> for AsyncLinked<T, F, D, Fin, DI> {
    /// In the sync table an async registration reports that it needs the async container.
    #[allow(private_interfaces)]
    fn walk(&self, entries: &mut Vec<Entry>) {
        entries.push(Entry {
            item: core::ptr::from_ref(self).cast(),
            construct: async_only::<T>,
            transient: async_only_transient::<T>,
            finalize: no_finalize,
        });
    }
}

/// Constructs a linked registration inside an async container.
pub trait AsyncExec<Root> {
    type Provides: 'static;

    fn construct_async<'a>(
        &'a self,
        root: &'a Root,
        container: &'a Container,
        index: usize,
    ) -> impl Future<Output = Result<Self::Provides, ResolveErrorKind>> + SendSafety + 'a;

    fn construct_async_shared<'a>(
        &'a self,
        root: &'a Root,
        container: &'a Container,
        index: usize,
    ) -> impl Future<Output = Result<RcAnyThreadSafety, ResolveErrorKind>> + SendSafety + 'a
    where
        Self::Provides: SendSafety + SyncSafety;
}

/// Sync leaves run their sync construction inside the async container.
macro_rules! sync_leaf_async_exec {
    ($($ty:ty $(, $param:ident)*;)*) => {
        $(impl<Root: SyncSafety $(, $param)*> AsyncExec<Root> for $ty
        where
            Self: Exec<Root> + SyncSafety,
            <Self as Exec<Root>>::Provides: SendSafety,
        {
            type Provides = <Self as Exec<Root>>::Provides;

            #[inline]
            fn construct_async<'a>(
                &'a self,
                root: &'a Root,
                container: &'a Container,
                index: usize,
            ) -> impl Future<Output = Result<Self::Provides, ResolveErrorKind>> + SendSafety + 'a {
                core::future::ready(self.construct(root, &container.sync, index))
            }

            #[inline]
            fn construct_async_shared<'a>(
                &'a self,
                root: &'a Root,
                container: &'a Container,
                index: usize,
            ) -> impl Future<Output = Result<RcAnyThreadSafety, ResolveErrorKind>> + SendSafety + 'a
            where
                Self::Provides: SendSafety + SyncSafety,
            {
                core::future::ready(self.construct_shared(root, &container.sync, index))
            }
        })*
    };
}

sync_leaf_async_exec! {
    crate::graph::Linked<T, F, D, Fin, DI>, T, F, D, Fin, DI;
    ContainerLeaf;
    crate::boundary::ImportLeaf<T>, T;
    crate::boundary::ContextLeaf<T>, T;
}

impl<Root, T, F, D, Fin, DI> AsyncExec<Root> for AsyncLinked<T, F, D, Fin, DI>
where
    Root: SyncSafety,
    T: SendSafety + 'static,
    F: Instantiator<D, Provides = T> + SendSafety + SyncSafety,
    D: AsyncDepsExec<Root, DI> + SendSafety,
    Fin: SyncSafety,
{
    type Provides = T;

    fn construct_async<'a>(
        &'a self,
        root: &'a Root,
        container: &'a Container,
        _index: usize,
    ) -> impl Future<Output = Result<T, ResolveErrorKind>> + SendSafety + 'a {
        async move {
            let dependencies = D::resolve_async(root, container)
                .await
                .map_err(|err| ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(err.into())))?;
            self.factory
                .clone()
                .instantiate(dependencies)
                .await
                .map_err(|err| ResolveErrorKind::Instantiator(InstantiatorErrorKind::Factory(err.into())))
        }
    }

    fn construct_async_shared<'a>(
        &'a self,
        root: &'a Root,
        container: &'a Container,
        index: usize,
    ) -> impl Future<Output = Result<RcAnyThreadSafety, ResolveErrorKind>> + SendSafety + 'a
    where
        T: SyncSafety,
    {
        async move { Ok(RcThreadSafety::new(self.construct_async(root, container, index).await?) as RcAnyThreadSafety) }
    }
}

/// Resolves one parameter of an async factory at its linked path.
#[diagnostic::on_unimplemented(
    message = "no registration provides the async factory parameter `{Self}`",
    label = "this registry has no `provide(...)` for the type inside `{Self}`"
)]
pub trait AsyncDepExec<Root, I>: Sized {
    fn resolve_async<'a>(
        root: &'a Root,
        container: &'a Container,
    ) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a;
}

impl<Root, T, I> AsyncDepExec<Root, SharedAt<I>> for Inject<T>
where
    Root: At<I> + SyncSafety,
    Root::Item: AsyncExec<Root, Provides = T> + SyncSafety,
    T: SendSafety + SyncSafety + 'static,
{
    fn resolve_async<'a>(
        root: &'a Root,
        container: &'a Container,
    ) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a {
        async move {
            let value = container
                .shared_with(Root::INDEX, |owner| root.at().construct_async_shared(root, owner, Root::INDEX))
                .await?;
            // SAFETY: slot `Root::INDEX` only holds values of the registration at path `I`,
            // which provides `T`.
            Ok(Inject(unsafe { downcast_unchecked(value) }))
        }
    }
}

impl<Root, T, I> AsyncDepExec<Root, TransientAt<I>> for InjectTransient<T>
where
    Root: At<I> + SyncSafety,
    Root::Item: AsyncExec<Root, Provides = T> + SyncSafety,
    T: SendSafety + 'static,
{
    fn resolve_async<'a>(
        root: &'a Root,
        container: &'a Container,
    ) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a {
        async move {
            if let Some(replacement) = container.sync.inner.plan.compiled.nodes()[Root::INDEX].replaced_by {
                return container.transient_at::<T>(replacement.index()).await.map(InjectTransient);
            }
            root.at().construct_async(root, container, Root::INDEX).await.map(InjectTransient)
        }
    }
}

impl<Root, R: DependencyResolver + SendSafety + 'static> AsyncDepExec<Root, ByResolver> for R {
    fn resolve_async<'a>(
        _root: &'a Root,
        container: &'a Container,
    ) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a {
        core::future::ready(R::resolve(&container.sync).map_err(Into::into))
    }
}

/// Resolves all parameters of an async factory, in order.
pub trait AsyncDepsExec<Root, I>: Sized {
    fn resolve_async<'a>(
        root: &'a Root,
        container: &'a Container,
    ) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a;
}

macro_rules! impl_async_deps_exec {
    ([$($dep:ident $index:ident),*]) => {
        impl<Root: SyncSafety, $($dep: AsyncDepExec<Root, $index> + SendSafety, $index,)*> AsyncDepsExec<Root, ($($index,)*)> for ($($dep,)*) {
            #[allow(unused_variables)]
            fn resolve_async<'a>(root: &'a Root, container: &'a Container) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a {
                async move { Ok(($(<$dep as AsyncDepExec<Root, $index>>::resolve_async(root, container).await?,)*)) }
            }
        }
    };
}

all_the_tuple_pairs!(impl_async_deps_exec);

type ConstructAsync =
    for<'a> unsafe fn(*const (), *const (), &'a Container, usize) -> BoxFuture<'a, Result<RcAnyThreadSafety, ResolveErrorKind>>;
type TransientAsync =
    for<'a> unsafe fn(*const (), *const (), &'a Container, usize) -> BoxFuture<'a, Result<BoxAnyThreadSafety, ResolveErrorKind>>;

/// Async construction functions of one registration.
pub(crate) struct AsyncEntry {
    item: *const (),
    construct: ConstructAsync,
    transient: TransientAsync,
    finalize: unsafe fn(*const (), RcAnyThreadSafety) -> BoxFuture<'static, ()>,
}

// SAFETY: `item` points into the boxed tree of the plan that owns this entry, which is
// `Send + Sync` in thread-safe builds; the other fields are plain function pointers.
#[cfg(feature = "thread_safe")]
unsafe impl Send for AsyncEntry {}
#[cfg(feature = "thread_safe")]
unsafe impl Sync for AsyncEntry {}

/// The async construction table of a plan, indexed by registration id. Registrations of
/// runtime registries have no entry and are constructed synchronously.
#[derive(Default)]
pub(crate) struct AsyncTable {
    entries: Vec<Option<AsyncEntry>>,
    #[cfg(feature = "thread_safe")]
    locks: Vec<tokio::sync::Mutex<()>>,
}

impl AsyncTable {
    pub(crate) fn fill(&mut self, len: usize) {
        self.entries.resize_with(len, || None);
        #[cfg(feature = "thread_safe")]
        self.locks.resize_with(len, || tokio::sync::Mutex::new(()));
    }
}

/// # Safety
/// `root` must point to a live `Root`, `item` to a live `Item` inside it.
unsafe fn construct_async_erased<Root: SyncSafety + 'static, Item>(
    root: *const (),
    item: *const (),
    container: &Container,
    index: usize,
) -> BoxFuture<'_, Result<RcAnyThreadSafety, ResolveErrorKind>>
where
    Item: AsyncExec<Root> + SyncSafety + 'static,
    Item::Provides: SendSafety + SyncSafety,
{
    // SAFETY: guaranteed by the caller; the plan outlives every container that uses it.
    let (root, item) = unsafe { (&*root.cast::<Root>(), &*item.cast::<Item>()) };
    Box::pin(item.construct_async_shared(root, container, index))
}

/// # Safety
/// As [`construct_async_erased`].
unsafe fn transient_async_erased<Root: SyncSafety + 'static, Item>(
    root: *const (),
    item: *const (),
    container: &Container,
    index: usize,
) -> BoxFuture<'_, Result<BoxAnyThreadSafety, ResolveErrorKind>>
where
    Item: AsyncExec<Root> + SyncSafety + 'static,
    Item::Provides: SendSafety + SyncSafety,
{
    // SAFETY: guaranteed by the caller.
    let (root, item) = unsafe { (&*root.cast::<Root>(), &*item.cast::<Item>()) };
    Box::pin(async move { Ok(Box::new(item.construct_async(root, container, index).await?) as BoxAnyThreadSafety) })
}

/// # Safety
/// `item` must point to a live `AsyncLinked<T, F, D, Fin, DI>`, and `value` must hold a `T`.
unsafe fn finalize_async_erased<T, F, D, Fin, DI>(item: *const (), value: RcAnyThreadSafety) -> BoxFuture<'static, ()>
where
    T: SendSafety + SyncSafety + 'static,
    Fin: MaybeFinalizer<T> + 'static,
{
    // SAFETY: guaranteed by the caller.
    let item = unsafe { &*item.cast::<AsyncLinked<T, F, D, Fin, DI>>() };
    // SAFETY: guaranteed by the caller.
    let value = unsafe { downcast_unchecked::<T>(value) };
    Box::pin(item.finalizer.clone().finalize(value))
}

/// # Safety
/// Never runs anything.
unsafe fn finalize_nothing(_item: *const (), _value: RcAnyThreadSafety) -> BoxFuture<'static, ()> {
    Box::pin(core::future::ready(()))
}

/// Collects the async construction entries of a linked tree, in declaration order.
pub trait AsyncWalk<Root> {
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn walk_async(&self, entries: &mut Vec<Option<AsyncEntry>>);
}

macro_rules! sync_leaf_async_walk {
    ($($ty:ty $(, $param:ident)*;)*) => {
        $(impl<Root: SyncSafety + 'static $(, $param)*> AsyncWalk<Root> for $ty
        where
            Self: AsyncExec<Root> + Exec<Root> + SyncSafety + 'static,
            <Self as AsyncExec<Root>>::Provides: SendSafety + SyncSafety,
        {
            #[allow(private_interfaces)]
            fn walk_async(&self, entries: &mut Vec<Option<AsyncEntry>>) {
                entries.push(Some(AsyncEntry {
                    item: core::ptr::from_ref(self).cast(),
                    construct: construct_async_erased::<Root, Self>,
                    transient: transient_async_erased::<Root, Self>,
                    finalize: finalize_nothing,
                }));
            }
        })*
    };
}

sync_leaf_async_walk! {
    crate::graph::Linked<T, F, D, Fin, DI>, T, F, D, Fin, DI;
    ContainerLeaf;
    crate::boundary::ImportLeaf<T>, T;
    crate::boundary::ContextLeaf<T>, T;
}

impl<Root, T, F, D, Fin, DI> AsyncWalk<Root> for AsyncLinked<T, F, D, Fin, DI>
where
    Root: SyncSafety + 'static,
    Self: AsyncExec<Root, Provides = T> + SyncSafety + 'static,
    T: SendSafety + SyncSafety + 'static,
    Fin: MaybeFinalizer<T> + 'static,
{
    #[allow(private_interfaces)]
    fn walk_async(&self, entries: &mut Vec<Option<AsyncEntry>>) {
        entries.push(Some(AsyncEntry {
            item: core::ptr::from_ref(self).cast(),
            construct: construct_async_erased::<Root, Self>,
            transient: transient_async_erased::<Root, Self>,
            finalize: finalize_async_erased::<T, F, D, Fin, DI>,
        }));
    }
}

impl<Root, A: AsyncWalk<Root>, B: AsyncWalk<Root>> AsyncWalk<Root> for Node<A, B> {
    #[allow(private_interfaces)]
    fn walk_async(&self, entries: &mut Vec<Option<AsyncEntry>>) {
        self.0.walk_async(entries);
        self.1.walk_async(entries);
    }
}

impl<Root> AsyncWalk<Root> for crate::graph::Empty {
    #[allow(private_interfaces)]
    fn walk_async(&self, _entries: &mut Vec<Option<AsyncEntry>>) {}
}

impl<Root> AsyncWalk<Root> for crate::runtime_registry::RuntimeNode {
    #[allow(private_interfaces)]
    fn walk_async(&self, _entries: &mut Vec<Option<AsyncEntry>>) {}
}

/// The async container. It is the sync [`container::Container`] with async construction.
#[derive(Clone)]
pub struct Container {
    pub(crate) sync: container::Container,
}

impl Container {
    /// Builds the async container of a registry.
    ///
    /// # Panics
    /// Panics with the rendered diagnostics if the registry graph is invalid.
    #[must_use]
    pub fn new<Tree, Links>(registry: Registry<Tree>) -> Self
    where
        Node<Tree, ContainerLeaf>: Link<Node<Tree, ContainerLeaf>, Links>,
        Linked<Tree, Links>: Walk<Linked<Tree, Links>>
            + AsyncWalk<Linked<Tree, Links>>
            + DescribeLinked<Linked<Tree, Links>>
            + CollectRuntime
            + SendSafety
            + SyncSafety
            + 'static,
    {
        Self::try_new(registry).unwrap_or_else(|diagnostics| panic!("invalid registry:\n{diagnostics}"))
    }

    /// Builds the async container of a registry after compiling its graph.
    ///
    /// # Errors
    /// Returns the graph compiler's diagnostics.
    pub fn try_new<Tree, Links>(registry: Registry<Tree>) -> Result<Self, Diagnostics>
    where
        Node<Tree, ContainerLeaf>: Link<Node<Tree, ContainerLeaf>, Links>,
        Linked<Tree, Links>: Walk<Linked<Tree, Links>>
            + AsyncWalk<Linked<Tree, Links>>
            + DescribeLinked<Linked<Tree, Links>>
            + CollectRuntime
            + SendSafety
            + SyncSafety
            + 'static,
    {
        let plan = Plan::build_with(registry, |tree| {
            let mut table = AsyncTable::default();
            tree.walk_async(&mut table.entries);
            table
        })?;
        Ok(Self {
            sync: container::Container::root(plan, |scope| !scope.is_skipped_by_default),
        })
    }

    /// Gets a scoped dependency.
    ///
    /// # Errors
    /// Returns [`ResolveErrorKind`] if nothing provides `Dep` or its construction fails.
    pub async fn get<Dep: SendSafety + SyncSafety + 'static>(&self) -> Result<RcThreadSafety<Dep>, ResolveErrorKind> {
        let type_info = TypeInfo::of::<Dep>();
        let value = match self.sync.inner.plan.compiled.lookup(&TypeId::of::<Dep>()) {
            Some(id) => self.shared(id.index()).await?,
            None => match self.sync.inner.context.map.get(&TypeId::of::<Dep>()) {
                Some(value) => value.clone(),
                None => return Err(ResolveErrorKind::NoInstantiator { type_info }),
            },
        };
        value.downcast::<Dep>().map_err(|value| ResolveErrorKind::IncorrectType {
            expected: type_info,
            actual: TypeInfo {
                name: "<unknown>",
                id: (*value).type_id(),
            },
        })
    }

    /// Gets a transient dependency: a new value on every call.
    ///
    /// # Errors
    /// Returns [`ResolveErrorKind`] if nothing provides `Dep` or its construction fails.
    pub async fn get_transient<Dep: 'static>(&self) -> Result<Dep, ResolveErrorKind> {
        let Some(id) = self.sync.inner.plan.compiled.lookup(&TypeId::of::<Dep>()) else {
            return Err(ResolveErrorKind::NoInstantiator {
                type_info: TypeInfo::of::<Dep>(),
            });
        };
        self.transient_at(id.index()).await
    }

    /// Closes the container: awaits the finalizers of the values constructed here since the last
    /// close, newest first, resets the cache, and closes the intermediate parents.
    pub async fn close(&self) {
        let mut container = self.sync.clone();
        loop {
            let resolved = core::mem::take(&mut *container.inner.resolved.write());
            let plan = &container.inner.plan;
            for (index, value) in resolved.into_iter().rev() {
                if let (Some(entry), Some(ExecutionKind::Async)) =
                    (&plan.async_table.entries[index], plan.compiled.nodes()[index].finalizer)
                {
                    // SAFETY: `value` was constructed by the registration at `index`.
                    unsafe { (entry.finalize)(entry.item, value) }.await;
                } else {
                    let entry = &plan.entries[index];
                    // SAFETY: `value` was constructed by the registration at `index`.
                    unsafe { (entry.finalize)(entry.item, value) };
                }
            }
            container.inner.reset_slots();
            match (&container.inner.parent, container.inner.close_parent) {
                (Some(parent), true) => container = parent.clone(),
                _ => break,
            }
        }
    }

    /// Creates a child container in the next scope that is not skipped by default.
    ///
    /// # Errors
    /// See [`container::ChildContainerBuilder::build`].
    pub fn enter_build(self) -> Result<Self, ScopeErrorKind> {
        self.sync.enter_build().map(|sync| Self { sync })
    }

    /// Creates a child container in the given scope.
    ///
    /// # Errors
    /// See [`container::ChildContainerWithScope::build`].
    pub fn enter_build_with_scope<S: Scope>(self, scope: S) -> Result<Self, ScopeWithErrorKind> {
        self.sync.enter().with_scope(scope).build().map(|sync| Self { sync })
    }

    #[inline]
    fn view(sync: &container::Container) -> Self {
        Self { sync: sync.clone() }
    }

    /// `get` semantics for the registration at `index`, through its async entry.
    fn shared(&self, index: usize) -> BoxFuture<'_, Result<RcAnyThreadSafety, ResolveErrorKind>> {
        let plan = self.sync.inner.plan.clone();
        Box::pin(async move {
            match &plan.async_table.entries[index] {
                Some(entry) => {
                    // SAFETY: the entry belongs to the tree `plan.root` points to.
                    self.shared_with(index, |owner| unsafe { (entry.construct)(plan.root, entry.item, owner, index) })
                        .await
                }
                None => self.sync.shared(index),
            }
        })
    }

    /// `get` semantics for the registration at `index`: the cached value, or the result of
    /// `construct` run in the owning container.
    pub(crate) async fn shared_with<'a, Fut>(
        &'a self,
        index: usize,
        construct: impl FnOnce(&'a Container) -> Fut,
    ) -> Result<RcAnyThreadSafety, ResolveErrorKind>
    where
        Fut: Future<Output = Result<RcAnyThreadSafety, ResolveErrorKind>> + 'a,
    {
        let inner = &self.sync.inner;
        let node = &inner.plan.compiled.nodes()[index];
        if let Some(replacement) = node.replaced_by {
            return self.shared(replacement.index()).await;
        }
        if let Some(value) = inner.slots.read().get(index) {
            return Ok(value.clone());
        }
        let value = match node.scope.cmp(&inner.level) {
            core::cmp::Ordering::Less => {
                // Resolve in the owning container, keep a copy here as Froodi does.
                let owner = Self::view(self.sync.ancestor(node.scope));
                owner.shared(index).await?
            }
            core::cmp::Ordering::Greater => {
                return Err(ResolveErrorKind::NoAccessible {
                    expected_scope_data: inner.plan.scopes[node.scope.index()],
                    actual_scope_data: self.sync.scope(),
                });
            }
            core::cmp::Ordering::Equal => {
                #[cfg(feature = "thread_safe")]
                let _guard = inner.plan.async_table.locks[index].lock().await;
                #[cfg(feature = "thread_safe")]
                if let Some(value) = inner.slots.read().get(index) {
                    return Ok(value.clone());
                }
                let value = construct(self).await?;
                if node.finalizer.is_some() {
                    inner.resolved.write().push((index, value.clone()));
                }
                value
            }
        };
        if node.cache_provides {
            inner.slots.write().set(index, value.clone(), inner.plan.entries.len());
        }
        Ok(value)
    }

    /// `get_transient` semantics for the registration at `index`, which must provide `Dep`.
    async fn transient_at<Dep: 'static>(&self, index: usize) -> Result<Dep, ResolveErrorKind> {
        let plan = self.sync.inner.plan.clone();
        let node = &plan.compiled.nodes()[index];
        if let Some(replacement) = node.replaced_by {
            return Box::pin(self.transient_at(replacement.index())).await;
        }
        let owner = match node.scope.cmp(&self.sync.inner.level) {
            core::cmp::Ordering::Greater => {
                return Err(ResolveErrorKind::NoAccessible {
                    expected_scope_data: plan.scopes[node.scope.index()],
                    actual_scope_data: self.sync.scope(),
                });
            }
            _ => Self::view(self.sync.ancestor(node.scope)),
        };
        let value = match &plan.async_table.entries[index] {
            // SAFETY: the entry belongs to the tree `plan.root` points to.
            Some(entry) => unsafe { (entry.transient)(plan.root, entry.item, &owner, index) }.await?,
            None => return owner.sync.transient_at::<Dep>(index),
        };
        value
            .downcast::<Dep>()
            .map(|value| *value)
            .map_err(|value| ResolveErrorKind::IncorrectType {
                expected: TypeInfo::of::<Dep>(),
                actual: TypeInfo {
                    name: "<unknown>",
                    id: (*value).type_id(),
                },
            })
    }
}
