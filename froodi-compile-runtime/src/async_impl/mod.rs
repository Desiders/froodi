//! Async registrations use the same linked tree, graph compiler and indexed executors.
//! Construction futures are boxed at the erased boundary; dependency bounds stay shallow.
//! Sync instantiators cannot depend on async registrations.

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
        ByResolver, CollectExecutors, CollectRuntime, ConstructRegistration, ContainerLeaf, DependenciesMetadata, Describe, DescribeLinked,
        Finalize, Here, Link, LinkDependencies, LinkedDependenciesMetadata, LinkedInject, LinkedInjectTransient, Meta, Node, ProviderPath,
        RegistrationExecutor, RegistrationPath, Size,
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

pub trait Instantiator<Deps>: Clone + 'static {
    type Provides: 'static;
    type Error: Into<InstantiateErrorKind>;

    fn instantiate(&mut self, dependencies: Deps) -> impl Future<Output = Result<Self::Provides, Self::Error>> + SendSafety;
}

macro_rules! impl_async_instantiator {
    ([$($ty:ident),*]) => {
        #[allow(non_snake_case)]
        impl<Inst, Fut, Out, Err, $($ty,)*> Instantiator<($($ty,)*)> for Inst
        where
            Inst: FnMut($($ty,)*) -> Fut + Clone + 'static,
            Fut: Future<Output = Result<Out, Err>> + SendSafety,
            Out: 'static,
            Err: Into<InstantiateErrorKind>,
        {
            type Provides = Out;
            type Error = Err;

            #[inline]
            fn instantiate(&mut self, ($($ty,)*): ($($ty,)*)) -> impl Future<Output = Result<Out, Err>> + SendSafety {
                self($($ty,)*)
            }
        }
    };
}

all_the_tuples!(impl_async_instantiator);

pub trait Finalizer<Dep>: Clone + 'static {
    fn finalize(&mut self, dependency: RcThreadSafety<Dep>) -> impl Future<Output = ()> + SendSafety;
}

impl<Fin, Fut, Dep> Finalizer<Dep> for Fin
where
    Fin: FnMut(RcThreadSafety<Dep>) -> Fut + Clone + 'static,
    Fut: Future<Output = ()> + SendSafety,
{
    #[inline]
    fn finalize(&mut self, dependency: RcThreadSafety<Dep>) -> impl Future<Output = ()> + SendSafety {
        self(dependency)
    }
}

/// Consumes a clone so the returned future owns the finalizer.
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

impl<Dep: SendSafety + SyncSafety, Fin: Finalizer<Dep> + SendSafety> MaybeFinalizer<Dep> for WithFinalizer<Fin> {
    const PRESENT: bool = true;

    #[inline]
    fn finalize(self, dependency: RcThreadSafety<Dep>) -> impl Future<Output = ()> + SendSafety {
        let mut finalizer = self.0;
        async move { finalizer.finalize(dependency).await }
    }
}

pub struct AsyncReg<Out, Inst, Deps, Fin> {
    instantiator: Inst,
    finalizer: Fin,
    meta: Meta,
    marker: PhantomData<fn() -> (Out, Deps)>,
}

#[doc(hidden)]
#[inline]
pub fn async_reg<S: Scope, Inst, Deps, Fin>(
    scope: S,
    instantiator: Inst,
    config: Option<Config>,
    finalizer: Fin,
    source: froodi_compile_core::ValueSource,
    origin: froodi_compile_core::Origin,
) -> AsyncReg<Inst::Provides, Inst, Deps, Fin>
where
    Inst: Instantiator<Deps>,
    Fin: MaybeFinalizer<Inst::Provides>,
{
    AsyncReg {
        instantiator,
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

impl<Out, Inst, Deps, Fin> Size for AsyncReg<Out, Inst, Deps, Fin> {
    const SIZE: usize = 1;
}

impl<Out, Inst, Deps, Fin> ProviderPath<Out, Here> for AsyncReg<Out, Inst, Deps, Fin> {}

impl<Out, Inst, Deps, Fin> CollectRuntime for AsyncReg<Out, Inst, Deps, Fin> {}

impl<Out: 'static, Inst, Deps: DependenciesMetadata, Fin: MaybeFinalizer<Out>> Describe for AsyncReg<Out, Inst, Deps, Fin> {
    fn describe(&self, out: &mut Vec<Registration<TypeId>>) {
        out.push(registration::<Out>(&self.meta, Fin::PRESENT, Deps::requests()));
    }
}

impl<Root, Out, Inst, Deps: LinkDependencies<Root, Links>, Fin, Links> Link<Root, Links> for AsyncReg<Out, Inst, Deps, Fin> {
    type Linked = AsyncLinked<Out, Inst, Deps, Fin, Links>;

    #[inline]
    fn link(self) -> Self::Linked {
        AsyncLinked {
            instantiator: self.instantiator,
            finalizer: self.finalizer,
            meta: self.meta,
            marker: PhantomData,
        }
    }
}

pub struct AsyncLinked<Out, Inst, Deps, Fin, Links> {
    instantiator: Inst,
    finalizer: Fin,
    meta: Meta,
    #[allow(clippy::type_complexity, reason = "keep the registration marker inline with its type parameters")]
    marker: PhantomData<fn() -> (Out, Deps, Links)>,
}

impl<Out, Inst, Deps, Fin, Links> Size for AsyncLinked<Out, Inst, Deps, Fin, Links> {
    const SIZE: usize = 1;
}

impl<Out, Inst, Deps, Fin, Links> RegistrationPath<Here> for AsyncLinked<Out, Inst, Deps, Fin, Links> {
    type Registration = Self;
    const INDEX: usize = 0;
}

impl<Out, Inst, Deps, Fin, Links> CollectRuntime for AsyncLinked<Out, Inst, Deps, Fin, Links> {}

impl<Root, Out: 'static, Inst, Deps: LinkedDependenciesMetadata<Root, Links>, Fin: MaybeFinalizer<Out>, Links> DescribeLinked<Root>
    for AsyncLinked<Out, Inst, Deps, Fin, Links>
{
    fn describe_linked(&self, out: &mut Vec<Registration<TypeId>>) {
        out.push(registration::<Out>(&self.meta, Fin::PRESENT, Deps::requests()));
    }
}

impl<Out, Inst, Deps, Fin, Links> Finalize for AsyncLinked<Out, Inst, Deps, Fin, Links> {
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

impl<Root, Out: 'static, Inst, Deps, Fin, Links> CollectExecutors<Root> for AsyncLinked<Out, Inst, Deps, Fin, Links> {
    /// In the sync table an async registration reports that it needs the async container.
    #[allow(private_interfaces)]
    fn collect_executors(&self, executors: &mut Vec<RegistrationExecutor>) {
        executors.push(RegistrationExecutor {
            registration: core::ptr::from_ref(self).cast(),
            construct: async_only::<Out>,
            construct_transient: async_only_transient::<Out>,
            finalize: no_finalize,
        });
    }
}

pub trait ConstructAsyncRegistration<Root> {
    type Provides: 'static;

    fn construct_async<'a>(
        &'a self,
        root: &'a Root,
        container: &'a Container,
        index: usize,
    ) -> impl Future<Output = Result<Self::Provides, ResolveErrorKind>> + SendSafety + 'a;

    fn construct_async_inject<'a>(
        &'a self,
        root: &'a Root,
        container: &'a Container,
        index: usize,
    ) -> impl Future<Output = Result<RcAnyThreadSafety, ResolveErrorKind>> + SendSafety + 'a
    where
        Self::Provides: SendSafety + SyncSafety;
}

macro_rules! sync_leaf_async_construction {
    ($($ty:ty $(, $param:ident)*;)*) => {
        $(impl<Root: SyncSafety $(, $param)*> ConstructAsyncRegistration<Root> for $ty
        where
            Self: ConstructRegistration<Root> + SyncSafety,
            <Self as ConstructRegistration<Root>>::Provides: SendSafety,
        {
            type Provides = <Self as ConstructRegistration<Root>>::Provides;

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
            fn construct_async_inject<'a>(
                &'a self,
                root: &'a Root,
                container: &'a Container,
                index: usize,
            ) -> impl Future<Output = Result<RcAnyThreadSafety, ResolveErrorKind>> + SendSafety + 'a
            where
                Self::Provides: SendSafety + SyncSafety,
            {
                core::future::ready(self.construct_inject(root, &container.sync, index))
            }
        })*
    };
}

sync_leaf_async_construction! {
    crate::graph::Linked<Out, Inst, Deps, Fin, Links>, Out, Inst, Deps, Fin, Links;
    ContainerLeaf;
    crate::boundary::ImportLeaf<T>, T;
    crate::boundary::ContextLeaf<T>, T;
}

impl<Root, Out, Inst, Deps, Fin, Links> ConstructAsyncRegistration<Root> for AsyncLinked<Out, Inst, Deps, Fin, Links>
where
    Root: SyncSafety,
    Out: SendSafety + 'static,
    Inst: Instantiator<Deps, Provides = Out> + SendSafety + SyncSafety,
    Deps: ResolveAsyncLinkedDependencies<Root, Links> + SendSafety,
    Fin: SyncSafety,
{
    type Provides = Out;

    fn construct_async<'a>(
        &'a self,
        root: &'a Root,
        container: &'a Container,
        _index: usize,
    ) -> impl Future<Output = Result<Out, ResolveErrorKind>> + SendSafety + 'a {
        async move {
            let dependencies = Deps::resolve_async(root, container)
                .await
                .map_err(|err| ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(err.into())))?;
            self.instantiator
                .clone()
                .instantiate(dependencies)
                .await
                .map_err(|err| ResolveErrorKind::Instantiator(InstantiatorErrorKind::Factory(err.into())))
        }
    }

    fn construct_async_inject<'a>(
        &'a self,
        root: &'a Root,
        container: &'a Container,
        index: usize,
    ) -> impl Future<Output = Result<RcAnyThreadSafety, ResolveErrorKind>> + SendSafety + 'a
    where
        Out: SyncSafety,
    {
        async move { Ok(RcThreadSafety::new(self.construct_async(root, container, index).await?) as RcAnyThreadSafety) }
    }
}

#[diagnostic::on_unimplemented(
    message = "no registration provides the async instantiator parameter `{Self}`",
    label = "this registry has no `provide(...)` for the type inside `{Self}`"
)]
pub trait ResolveAsyncLinkedDependency<Root, Path>: Sized {
    fn resolve_async<'a>(
        root: &'a Root,
        container: &'a Container,
    ) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a;
}

impl<Root, T, Path> ResolveAsyncLinkedDependency<Root, LinkedInject<Path>> for Inject<T>
where
    Root: RegistrationPath<Path> + SyncSafety,
    T: SendSafety + SyncSafety + 'static,
{
    fn resolve_async<'a>(
        _root: &'a Root,
        container: &'a Container,
    ) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a {
        async move {
            let value = container.get_at(Root::INDEX).await?;
            // SAFETY: slot `Root::INDEX` only holds values of the registration at path `Path`,
            // which provides `T`.
            Ok(Inject(unsafe { downcast_unchecked(value) }))
        }
    }
}

impl<Root, T, Path> ResolveAsyncLinkedDependency<Root, LinkedInjectTransient<Path>> for InjectTransient<T>
where
    Root: RegistrationPath<Path> + SyncSafety,
    T: SendSafety + 'static,
{
    fn resolve_async<'a>(
        _root: &'a Root,
        container: &'a Container,
    ) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a {
        async move { container.get_transient_at::<T>(Root::INDEX).await.map(InjectTransient) }
    }
}

impl<Root, R: DependencyResolver + SendSafety + 'static> ResolveAsyncLinkedDependency<Root, ByResolver> for R {
    fn resolve_async<'a>(
        _root: &'a Root,
        container: &'a Container,
    ) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a {
        core::future::ready(R::resolve(&container.sync).map_err(Into::into))
    }
}

pub trait ResolveAsyncLinkedDependencies<Root, Links>: Sized {
    fn resolve_async<'a>(
        root: &'a Root,
        container: &'a Container,
    ) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a;
}

macro_rules! impl_resolve_async_linked_dependencies {
    ([$($dep:ident $index:ident),*]) => {
        impl<Root: SyncSafety, $($dep: ResolveAsyncLinkedDependency<Root, $index> + SendSafety, $index,)*> ResolveAsyncLinkedDependencies<Root, ($($index,)*)> for ($($dep,)*) {
            #[allow(unused_variables)]
            fn resolve_async<'a>(root: &'a Root, container: &'a Container) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a {
                async move { Ok(($(<$dep as ResolveAsyncLinkedDependency<Root, $index>>::resolve_async(root, container).await?,)*)) }
            }
        }
    };
}

all_the_tuple_pairs!(impl_resolve_async_linked_dependencies);

type ConstructAsync =
    for<'a> unsafe fn(*const (), *const (), &'a Container, usize) -> BoxFuture<'a, Result<RcAnyThreadSafety, ResolveErrorKind>>;
type TransientAsync =
    for<'a> unsafe fn(*const (), *const (), &'a Container, usize) -> BoxFuture<'a, Result<BoxAnyThreadSafety, ResolveErrorKind>>;

/// Shares [`RegistrationExecutor`]'s invariants; construction futures borrow plan storage.
pub(crate) struct AsyncRegistrationExecutor {
    registration: *const (),
    construct: ConstructAsync,
    construct_transient: TransientAsync,
    finalize: unsafe fn(*const (), RcAnyThreadSafety) -> BoxFuture<'static, ()>,
}

// SAFETY: `registration` points into the boxed tree of the plan that owns this executor, which is
// `Send + Sync` in thread-safe builds; the other fields are plain function pointers.
#[cfg(feature = "thread_safe")]
unsafe impl Send for AsyncRegistrationExecutor {}
#[cfg(feature = "thread_safe")]
unsafe impl Sync for AsyncRegistrationExecutor {}

/// Runtime registrations have no async executor and fall back to synchronous construction.
#[derive(Default)]
pub(crate) struct AsyncTable {
    executors: Vec<Option<AsyncRegistrationExecutor>>,
    #[cfg(feature = "thread_safe")]
    locks: Vec<tokio::sync::Mutex<()>>,
}

impl AsyncTable {
    pub(crate) fn fill(&mut self, len: usize) {
        self.executors.resize_with(len, || None);
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
    Item: ConstructAsyncRegistration<Root> + SyncSafety + 'static,
    Item::Provides: SendSafety + SyncSafety,
{
    // SAFETY: matching `Root`/`Item` pointers remain live through the container-borrowing future.
    let (root, item) = unsafe { (&*root.cast::<Root>(), &*item.cast::<Item>()) };
    Box::pin(item.construct_async_inject(root, container, index))
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
    Item: ConstructAsyncRegistration<Root> + SyncSafety + 'static,
    Item::Provides: SendSafety + SyncSafety,
{
    // SAFETY: matching `Root`/`Item` pointers stay live for the returned future.
    let (root, item) = unsafe { (&*root.cast::<Root>(), &*item.cast::<Item>()) };
    Box::pin(async move { Ok(Box::new(item.construct_async(root, container, index).await?) as BoxAnyThreadSafety) })
}

/// # Safety
/// `item` must point to a live `AsyncLinked<Out, Inst, Deps, Fin, Links>`, and `value` must have been provided by it.
unsafe fn finalize_async_erased<Out, Inst, Deps, Fin, Links>(item: *const (), value: RcAnyThreadSafety) -> BoxFuture<'static, ()>
where
    Out: SendSafety + SyncSafety + 'static,
    Fin: MaybeFinalizer<Out> + 'static,
{
    // SAFETY: the caller supplies a live `AsyncLinked<Out, Inst, Deps, Fin, Links>` with this executor's registration ID.
    let item = unsafe { &*item.cast::<AsyncLinked<Out, Inst, Deps, Fin, Links>>() };
    // SAFETY: the value was produced by this registration, whose provided type is `Out`.
    let value = unsafe { downcast_unchecked::<Out>(value) };
    Box::pin(item.finalizer.clone().finalize(value))
}

/// # Safety
/// Never runs anything.
unsafe fn finalize_nothing(_item: *const (), _value: RcAnyThreadSafety) -> BoxFuture<'static, ()> {
    Box::pin(core::future::ready(()))
}

/// Must use the same registration order as the sync executor table.
pub trait CollectAsyncExecutors<Root> {
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn collect_async_executors(&self, executors: &mut Vec<Option<AsyncRegistrationExecutor>>);
}

macro_rules! collect_sync_leaf_async_executors {
    ($($ty:ty $(, $param:ident)*;)*) => {
        $(impl<Root: SyncSafety + 'static $(, $param)*> CollectAsyncExecutors<Root> for $ty
        where
            Self: ConstructAsyncRegistration<Root> + ConstructRegistration<Root> + SyncSafety + 'static,
            <Self as ConstructAsyncRegistration<Root>>::Provides: SendSafety + SyncSafety,
        {
            #[allow(private_interfaces)]
            fn collect_async_executors(&self, executors: &mut Vec<Option<AsyncRegistrationExecutor>>) {
                executors.push(Some(AsyncRegistrationExecutor {
                    registration: core::ptr::from_ref(self).cast(),
                    construct: construct_async_erased::<Root, Self>,
                    construct_transient: transient_async_erased::<Root, Self>,
                    finalize: finalize_nothing,
                }));
            }
        })*
    };
}

collect_sync_leaf_async_executors! {
    crate::graph::Linked<Out, Inst, Deps, Fin, Links>, Out, Inst, Deps, Fin, Links;
    ContainerLeaf;
    crate::boundary::ImportLeaf<T>, T;
    crate::boundary::ContextLeaf<T>, T;
}

impl<Root, Out, Inst, Deps, Fin, Links> CollectAsyncExecutors<Root> for AsyncLinked<Out, Inst, Deps, Fin, Links>
where
    Root: SyncSafety + 'static,
    Self: ConstructAsyncRegistration<Root, Provides = Out> + SyncSafety + 'static,
    Out: SendSafety + SyncSafety + 'static,
    Fin: MaybeFinalizer<Out> + 'static,
{
    #[allow(private_interfaces)]
    fn collect_async_executors(&self, executors: &mut Vec<Option<AsyncRegistrationExecutor>>) {
        executors.push(Some(AsyncRegistrationExecutor {
            registration: core::ptr::from_ref(self).cast(),
            construct: construct_async_erased::<Root, Self>,
            construct_transient: transient_async_erased::<Root, Self>,
            finalize: finalize_async_erased::<Out, Inst, Deps, Fin, Links>,
        }));
    }
}

impl<Root, Left: CollectAsyncExecutors<Root>, Right: CollectAsyncExecutors<Root>> CollectAsyncExecutors<Root> for Node<Left, Right> {
    #[allow(private_interfaces)]
    fn collect_async_executors(&self, executors: &mut Vec<Option<AsyncRegistrationExecutor>>) {
        self.0.collect_async_executors(executors);
        self.1.collect_async_executors(executors);
    }
}

impl<Root> CollectAsyncExecutors<Root> for crate::graph::Empty {
    #[allow(private_interfaces)]
    fn collect_async_executors(&self, _executors: &mut Vec<Option<AsyncRegistrationExecutor>>) {}
}

impl<Root> CollectAsyncExecutors<Root> for crate::runtime_registry::RuntimeNode {
    #[allow(private_interfaces)]
    fn collect_async_executors(&self, _executors: &mut Vec<Option<AsyncRegistrationExecutor>>) {}
}

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
        Linked<Tree, Links>: CollectExecutors<Linked<Tree, Links>>
            + CollectAsyncExecutors<Linked<Tree, Links>>
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
        Linked<Tree, Links>: CollectExecutors<Linked<Tree, Links>>
            + CollectAsyncExecutors<Linked<Tree, Links>>
            + DescribeLinked<Linked<Tree, Links>>
            + CollectRuntime
            + SendSafety
            + SyncSafety
            + 'static,
    {
        let plan = Plan::build_with(registry, |tree| {
            let mut table = AsyncTable::default();
            tree.collect_async_executors(&mut table.executors);
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
            Some(id) => self.get_at(id.index()).await?,
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
        self.get_transient_at(id.index()).await
    }

    /// Closes the container: awaits the finalizers of the values constructed here since the last
    /// close, newest first, resets the cache, and closes the intermediate parents.
    pub async fn close(&self) {
        let mut container = self.sync.clone();
        loop {
            let resolved = core::mem::take(&mut *container.inner.resolved.write());
            let plan = &container.inner.plan;
            for (index, value) in resolved.into_iter().rev() {
                if let (Some(executor), Some(ExecutionKind::Async)) =
                    (&plan.async_table.executors[index], plan.compiled.nodes()[index].finalizer)
                {
                    // SAFETY: `value` was constructed by the registration at `index`.
                    unsafe { (executor.finalize)(executor.registration, value) }.await;
                } else {
                    let executor = &plan.executors[index];
                    // SAFETY: `value` was constructed by the registration at `index`.
                    unsafe { (executor.finalize)(executor.registration, value) };
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
    /// See `container::ChildContainerBuilder::build`.
    pub fn enter_build(self) -> Result<Self, ScopeErrorKind> {
        self.sync.enter_build().map(|sync| Self { sync })
    }

    /// Creates a child container in the given scope.
    ///
    /// # Errors
    /// See `container::ChildContainerWithScope::build`.
    pub fn enter_build_with_scope<S: Scope>(self, scope: S) -> Result<Self, ScopeWithErrorKind> {
        self.sync.enter().with_scope(scope).build().map(|sync| Self { sync })
    }

    #[inline]
    fn view(sync: &container::Container) -> Self {
        Self { sync: sync.clone() }
    }

    fn get_at(&self, index: usize) -> BoxFuture<'_, Result<RcAnyThreadSafety, ResolveErrorKind>> {
        Box::pin(async move {
            if self.sync.inner.plan.async_table.executors[index].is_none() {
                return self.sync.get_at(index);
            }
            let inner = &self.sync.inner;
            let node = &inner.plan.compiled.nodes()[index];
            if let Some(replacement) = node.replaced_by {
                return self.get_at(replacement.index()).await;
            }
            if let Some(value) = inner.slots.read().get(index) {
                return Ok(value.clone());
            }
            let value = match node.scope.cmp(&inner.level) {
                core::cmp::Ordering::Less => {
                    // Resolve in the owning container, keep a copy here as Froodi does.
                    let owner = Self::view(self.sync.ancestor(node.scope));
                    owner.get_at(index).await?
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
                    let executor = inner.plan.async_table.executors[index]
                        .as_ref()
                        .expect("async executor checked above");
                    // SAFETY: the plan and its boxed tree outlive this awaited call.
                    let value = unsafe { (executor.construct)(inner.plan.root, executor.registration, self, index) }.await?;
                    if node.finalizer.is_some() {
                        inner.resolved.write().push((index, value.clone()));
                    }
                    value
                }
            };
            if node.cache_provides {
                inner.slots.write().set(index, value.clone(), inner.plan.executors.len());
            }
            Ok(value)
        })
    }

    /// `get_transient` semantics for the registration at `index`, which must provide `Dep`.
    async fn get_transient_at<Dep: 'static>(&self, index: usize) -> Result<Dep, ResolveErrorKind> {
        let plan = self.sync.inner.plan.clone();
        let node = &plan.compiled.nodes()[index];
        if let Some(replacement) = node.replaced_by {
            return Box::pin(self.get_transient_at(replacement.index())).await;
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
        let value = match &plan.async_table.executors[index] {
            // SAFETY: the executor belongs to the tree `plan.root` points to.
            Some(executor) => unsafe { (executor.construct_transient)(plan.root, executor.registration, &owner, index) }.await?,
            None => return owner.sync.get_transient_at::<Dep>(index),
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
