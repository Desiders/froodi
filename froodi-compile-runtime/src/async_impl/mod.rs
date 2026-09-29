//! Async registrations use the same linked tree, graph compiler and indexed executors.
//! Construction futures are boxed at the erased boundary; dependency bounds stay shallow.
//! Sync instantiators cannot depend on async registrations.

#![allow(
    clippy::manual_async_fn,
    reason = "trait signatures state the `SendSafety` bound on returned futures"
)]

use alloc::{boxed::Box, vec::Vec};
use core::{any::TypeId, future::Future, marker::PhantomData, pin::Pin, slice};

use froodi_compile_core::{CompiledEdge, Diagnostics, ExecutionKind, Registration};

use crate::{
    config::Config,
    container::{self, Linked, Plan, ProviderIndex},
    dependency_resolver::DependencyResolver,
    errors::{InstantiateErrorKind, InstantiatorErrorKind, ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind, TypeInfo},
    finalizer::{NoFinalizer, WithFinalizer},
    graph::{
        next_edge, AsyncExecution, CollectExecutors, CollectRegistrations, CollectRuntime, ConstructRegistration, ContainerLeaf,
        DependenciesMetadata, Describe, Finalize, Here, Link, LinkDependencies, Meta, Node, ProviderPath, RegistrationExecutor,
        RegistryIndex, Size, SupportsExecution,
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

impl<Out, Inst, Deps, Fin> CollectRuntime for AsyncReg<Out, Inst, Deps, Fin> {}

impl<Out: 'static, Inst, Deps: DependenciesMetadata, Fin: MaybeFinalizer<Out>> Describe for AsyncReg<Out, Inst, Deps, Fin> {
    fn describe(&self, out: &mut Vec<Registration<TypeId>>) {
        out.push(registration::<Out>(&self.meta, Fin::PRESENT, Deps::requests()));
    }
}

pub struct AsyncProvider<Out>(PhantomData<fn() -> Out>);
impl<Out> Size for AsyncProvider<Out> {
    const SIZE: usize = 1;
}
impl<Out> ProviderPath<Out, Here> for AsyncProvider<Out> {
    type Provider = Self;
    const INDEX: usize = 0;
}
impl<Out> SupportsExecution<AsyncExecution> for AsyncProvider<Out> {}
impl<Out, Inst, Deps, Fin> RegistryIndex for AsyncReg<Out, Inst, Deps, Fin> {
    type Index = AsyncProvider<Out>;
}

impl<Root, Out, Inst, Deps, Fin, Links> Link<Root, Links> for AsyncReg<Out, Inst, Deps, Fin>
where
    Deps: LinkDependencies<Root, Links>,
    Deps::Providers: SupportsExecution<AsyncExecution>,
{
    type Linked = AsyncLinked<Out, Inst, Deps, Fin>;

    #[inline]
    fn link(self) -> Self::Linked {
        AsyncLinked {
            instantiator: self.instantiator,
            finalizer: self.finalizer,
            meta: self.meta,
            dependencies: Deps::requests(),
            marker: PhantomData,
        }
    }
}

pub struct AsyncLinked<Out, Inst, Deps, Fin> {
    instantiator: Inst,
    finalizer: Fin,
    meta: Meta,
    dependencies: Vec<froodi_compile_core::DependencyRequest<TypeId>>,
    marker: PhantomData<fn() -> (Out, Deps)>,
}

impl<Out, Inst, Deps, Fin> CollectRuntime for AsyncLinked<Out, Inst, Deps, Fin> {}

impl<Out: 'static, Inst, Deps, Fin: MaybeFinalizer<Out>> CollectRegistrations for AsyncLinked<Out, Inst, Deps, Fin> {
    fn collect_registrations(&mut self, out: &mut Vec<Registration<TypeId>>) {
        out.push(registration::<Out>(
            &self.meta,
            Fin::PRESENT,
            core::mem::take(&mut self.dependencies),
        ));
    }
}

impl<Out, Inst, Deps, Fin> Finalize for AsyncLinked<Out, Inst, Deps, Fin> {
    /// Async finalizers run only from the async container's `close().await`.
    unsafe fn finalize(&self, _value: RcAnyThreadSafety) {}
}

/// # Safety
/// Never dereferences its pointers.
unsafe fn async_only<T: 'static>(
    _item: *const (),
    _container: &container::Container,
    _edges: &[CompiledEdge],
) -> Result<RcAnyThreadSafety, ResolveErrorKind> {
    Err(ResolveErrorKind::AsyncOnly {
        type_info: TypeInfo::of::<T>(),
    })
}

/// # Safety
/// Never dereferences its pointers.
unsafe fn async_only_transient<T: 'static>(
    _item: *const (),
    _container: &container::Container,
    _edges: &[CompiledEdge],
    _out: *mut (),
) -> Result<(), ResolveErrorKind> {
    Err(ResolveErrorKind::AsyncOnly {
        type_info: TypeInfo::of::<T>(),
    })
}

/// # Safety
/// Never dereferences its pointers.
unsafe fn no_finalize(_item: *const (), _value: RcAnyThreadSafety) {}

impl<Out: 'static, Inst, Deps, Fin> CollectExecutors for AsyncLinked<Out, Inst, Deps, Fin> {
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

pub trait ConstructAsyncRegistration {
    type Provides: 'static;

    fn construct_async<'a>(
        &'a self,
        container: &'a Container,
        edges: &'a [CompiledEdge],
    ) -> impl Future<Output = Result<Self::Provides, ResolveErrorKind>> + SendSafety + 'a;

    fn construct_async_inject<'a>(
        &'a self,
        container: &'a Container,
        edges: &'a [CompiledEdge],
    ) -> impl Future<Output = Result<RcAnyThreadSafety, ResolveErrorKind>> + SendSafety + 'a
    where
        Self::Provides: SendSafety + SyncSafety;
}

macro_rules! sync_leaf_async_construction {
    ($($ty:ty $(, $param:ident)*;)*) => {
        $(impl<$($param,)*> ConstructAsyncRegistration for $ty
        where
            Self: ConstructRegistration + SyncSafety,
            <Self as ConstructRegistration>::Provides: SendSafety,
        {
            type Provides = <Self as ConstructRegistration>::Provides;

            #[inline]
            fn construct_async<'a>(
                &'a self,
                container: &'a Container,
                edges: &'a [CompiledEdge],
            ) -> impl Future<Output = Result<Self::Provides, ResolveErrorKind>> + SendSafety + 'a {
                core::future::ready(self.construct(&container.sync, edges))
            }

            #[inline]
            fn construct_async_inject<'a>(
                &'a self,
                container: &'a Container,
                edges: &'a [CompiledEdge],
            ) -> impl Future<Output = Result<RcAnyThreadSafety, ResolveErrorKind>> + SendSafety + 'a
            where
                Self::Provides: SendSafety + SyncSafety,
            {
                core::future::ready(self.construct_inject(&container.sync, edges))
            }
        })*
    };
}

sync_leaf_async_construction! {
    crate::graph::Linked<Out, Inst, Deps, Fin>, Out, Inst, Deps, Fin;
    ContainerLeaf;
    crate::boundary::ImportLeaf<T>, T;
    crate::boundary::ContextLeaf<T>, T;
}

impl<Out, Inst, Deps, Fin> ConstructAsyncRegistration for AsyncLinked<Out, Inst, Deps, Fin>
where
    Out: SendSafety + 'static,
    Inst: Instantiator<Deps, Provides = Out> + SendSafety + SyncSafety,
    Deps: ResolveAsyncLinkedDependencies + SendSafety,
    Fin: SyncSafety,
{
    type Provides = Out;

    fn construct_async<'a>(
        &'a self,
        container: &'a Container,
        edges: &'a [CompiledEdge],
    ) -> impl Future<Output = Result<Out, ResolveErrorKind>> + SendSafety + 'a {
        async move {
            // SAFETY: dispatch supplies this registration's compiled parameter edges.
            let dependencies = unsafe { Deps::resolve_async(container, &mut edges.iter()) }
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
        container: &'a Container,
        edges: &'a [CompiledEdge],
    ) -> impl Future<Output = Result<RcAnyThreadSafety, ResolveErrorKind>> + SendSafety + 'a
    where
        Out: SyncSafety,
    {
        async move { Ok(RcThreadSafety::new(self.construct_async(container, edges).await?) as RcAnyThreadSafety) }
    }
}

pub trait ResolveAsyncLinkedDependency: Sized {
    /// # Safety
    /// The next edge must provide this parameter's exact type; resolvers consume no edge.
    unsafe fn resolve_async<'a>(
        container: &'a Container,
        edges: &mut slice::Iter<'_, froodi_compile_core::CompiledEdge>,
    ) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a;
}

impl<T: SendSafety + SyncSafety + 'static> ResolveAsyncLinkedDependency for Inject<T> {
    unsafe fn resolve_async<'a>(
        container: &'a Container,
        edges: &mut slice::Iter<'_, froodi_compile_core::CompiledEdge>,
    ) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a {
        // SAFETY: the caller supplies this parameter's next compiled edge.
        let target = unsafe { next_edge(edges) }.target.index();
        async move {
            let value = container.get_at(target).await?;
            // SAFETY: linking proved the supplied edge provides T, including after redirects.
            Ok(Inject(unsafe { downcast_unchecked(value) }))
        }
    }
}

impl<T: SendSafety + 'static> ResolveAsyncLinkedDependency for InjectTransient<T> {
    unsafe fn resolve_async<'a>(
        container: &'a Container,
        edges: &mut slice::Iter<'_, froodi_compile_core::CompiledEdge>,
    ) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a {
        // SAFETY: the caller supplies this parameter's next compiled edge.
        let target = unsafe { next_edge(edges) }.target.index();
        async move { container.get_transient_at::<T>(target).await.map(InjectTransient) }
    }
}

impl<Resolver: DependencyResolver + SendSafety + 'static> ResolveAsyncLinkedDependency for Resolver {
    unsafe fn resolve_async<'a>(
        container: &'a Container,
        _edges: &mut slice::Iter<'_, froodi_compile_core::CompiledEdge>,
    ) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a {
        core::future::ready(Resolver::resolve(&container.sync).map_err(Into::into))
    }
}

pub trait ResolveAsyncLinkedDependencies: Sized {
    /// # Safety
    /// Edges must match the dependency tuple in parameter order, excluding custom resolvers.
    unsafe fn resolve_async<'a>(
        container: &'a Container,
        edges: &'a mut slice::Iter<'_, froodi_compile_core::CompiledEdge>,
    ) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a;
}

macro_rules! impl_resolve_async_linked_dependencies {
    ([$($dep:ident),*]) => {
        impl<$($dep: ResolveAsyncLinkedDependency + SendSafety,)*> ResolveAsyncLinkedDependencies for ($($dep,)*) {
            #[allow(unused_variables)]
            unsafe fn resolve_async<'a>(
                container: &'a Container,
                edges: &'a mut slice::Iter<'_, froodi_compile_core::CompiledEdge>,
            ) -> impl Future<Output = Result<Self, ResolveErrorKind>> + SendSafety + 'a {
                // SAFETY: parameters consume matching edges in order, skipping custom resolvers.
                async move { Ok(($(unsafe { $dep::resolve_async(container, edges) }.await?,)*)) }
            }
        }
    };
}

all_the_tuples!(impl_resolve_async_linked_dependencies);

type ConstructAsync =
    for<'a> unsafe fn(*const (), &'a Container, &'a [CompiledEdge]) -> BoxFuture<'a, Result<RcAnyThreadSafety, ResolveErrorKind>>;
type TransientAsync =
    for<'a> unsafe fn(*const (), &'a Container, &'a [CompiledEdge]) -> BoxFuture<'a, Result<BoxAnyThreadSafety, ResolveErrorKind>>;

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
/// `item` must point to a live `Item` with matching compiled `edges` in the container plan.
unsafe fn construct_async_erased<'a, Item>(
    item: *const (),
    container: &'a Container,
    edges: &'a [CompiledEdge],
) -> BoxFuture<'a, Result<RcAnyThreadSafety, ResolveErrorKind>>
where
    Item: ConstructAsyncRegistration + SyncSafety + 'static,
    Item::Provides: SendSafety + SyncSafety,
{
    // SAFETY: the matching `Item` pointer remains live through the container-borrowing future.
    let item = unsafe { &*item.cast::<Item>() };
    Box::pin(item.construct_async_inject(container, edges))
}

/// # Safety
/// As [`construct_async_erased`].
unsafe fn transient_async_erased<'a, Item>(
    item: *const (),
    container: &'a Container,
    edges: &'a [CompiledEdge],
) -> BoxFuture<'a, Result<BoxAnyThreadSafety, ResolveErrorKind>>
where
    Item: ConstructAsyncRegistration + SyncSafety + 'static,
    Item::Provides: SendSafety + SyncSafety,
{
    // SAFETY: the matching `Item` pointer stay live for the returned future.
    let item = unsafe { &*item.cast::<Item>() };
    Box::pin(async move { Ok(Box::new(item.construct_async(container, edges).await?) as BoxAnyThreadSafety) })
}

/// # Safety
/// `item` must point to a live `AsyncLinked<Out, Inst, Deps, Fin>`, and `value` must have been provided by it.
unsafe fn finalize_async_erased<Out, Inst, Deps, Fin>(item: *const (), value: RcAnyThreadSafety) -> BoxFuture<'static, ()>
where
    Out: SendSafety + SyncSafety + 'static,
    Fin: MaybeFinalizer<Out> + 'static,
{
    // SAFETY: the caller supplies a live `AsyncLinked<Out, Inst, Deps, Fin>` with this executor's registration ID.
    let item = unsafe { &*item.cast::<AsyncLinked<Out, Inst, Deps, Fin>>() };
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
pub trait CollectAsyncExecutors {
    #[doc(hidden)]
    #[allow(private_interfaces)]
    fn collect_async_executors(&self, executors: &mut Vec<Option<AsyncRegistrationExecutor>>);
}

macro_rules! collect_sync_leaf_async_executors {
    ($($ty:ty $(, $param:ident)*;)*) => {
        $(impl<$($param,)*> CollectAsyncExecutors for $ty
        where
            Self: ConstructAsyncRegistration + ConstructRegistration + SyncSafety + 'static,
            <Self as ConstructAsyncRegistration>::Provides: SendSafety + SyncSafety,
        {
            #[allow(private_interfaces)]
            fn collect_async_executors(&self, executors: &mut Vec<Option<AsyncRegistrationExecutor>>) {
                executors.push(Some(AsyncRegistrationExecutor {
                    registration: core::ptr::from_ref(self).cast(),
                    construct: construct_async_erased::<Self>,
                    construct_transient: transient_async_erased::<Self>,
                    finalize: finalize_nothing,
                }));
            }
        })*
    };
}

collect_sync_leaf_async_executors! {
    crate::graph::Linked<Out, Inst, Deps, Fin>, Out, Inst, Deps, Fin;
    ContainerLeaf;
    crate::boundary::ImportLeaf<T>, T;
    crate::boundary::ContextLeaf<T>, T;
}

impl<Out, Inst, Deps, Fin> CollectAsyncExecutors for AsyncLinked<Out, Inst, Deps, Fin>
where
    Self: ConstructAsyncRegistration<Provides = Out> + SyncSafety + 'static,
    Out: SendSafety + SyncSafety + 'static,
    Fin: MaybeFinalizer<Out> + 'static,
{
    #[allow(private_interfaces)]
    fn collect_async_executors(&self, executors: &mut Vec<Option<AsyncRegistrationExecutor>>) {
        executors.push(Some(AsyncRegistrationExecutor {
            registration: core::ptr::from_ref(self).cast(),
            construct: construct_async_erased::<Self>,
            construct_transient: transient_async_erased::<Self>,
            finalize: finalize_async_erased::<Out, Inst, Deps, Fin>,
        }));
    }
}

impl<Left: CollectAsyncExecutors, Right: CollectAsyncExecutors> CollectAsyncExecutors for Node<Left, Right> {
    #[allow(private_interfaces)]
    fn collect_async_executors(&self, executors: &mut Vec<Option<AsyncRegistrationExecutor>>) {
        self.0.collect_async_executors(executors);
        self.1.collect_async_executors(executors);
    }
}

impl CollectAsyncExecutors for crate::graph::Empty {
    #[allow(private_interfaces)]
    fn collect_async_executors(&self, _executors: &mut Vec<Option<AsyncRegistrationExecutor>>) {}
}

impl CollectAsyncExecutors for crate::runtime_registry::RuntimeNode {
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
        Node<Tree, ContainerLeaf>: RegistryIndex + Link<ProviderIndex<Tree>, Links>,
        Linked<Tree, Links>:
            CollectExecutors + CollectAsyncExecutors + CollectRegistrations + CollectRuntime + SendSafety + SyncSafety + 'static,
    {
        Self::try_new(registry).unwrap_or_else(|diagnostics| panic!("invalid registry:\n{diagnostics}"))
    }

    /// Builds the async container of a registry after compiling its graph.
    ///
    /// # Errors
    /// Returns the graph compiler's diagnostics.
    pub fn try_new<Tree, Links>(registry: Registry<Tree>) -> Result<Self, Diagnostics>
    where
        Node<Tree, ContainerLeaf>: RegistryIndex + Link<ProviderIndex<Tree>, Links>,
        Linked<Tree, Links>:
            CollectExecutors + CollectAsyncExecutors + CollectRegistrations + CollectRuntime + SendSafety + SyncSafety + 'static,
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
                    let value = unsafe { (executor.construct)(executor.registration, self, &node.edges) }.await?;
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
            // SAFETY: the executor matches this registration and the owner retains its plan storage.
            Some(executor) => unsafe { (executor.construct_transient)(executor.registration, &owner, &node.edges) }.await?,
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
