use alloc::vec::Vec;
use core::any::TypeId;

use core::cmp::Ordering;

#[cfg(feature = "thread_safe")]
use crate::lock::NodeLocks;

use froodi_compile_core::{compile, CompiledGraph, Diagnostics, ExecutionKind, ScopeId};

use crate::{
    context::Context,
    errors::{ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind, TypeInfo},
    graph::{CollectExecutors, CollectRegistrations, CollectRuntime, ContainerLeaf, Link, Node, RegistrationExecutor, RegistryIndex},
    lock::LocalLock,
    registry::Registry,
    scope::{Scope, ScopeData},
    thread_safety::{RcAnyThreadSafety, RcThreadSafety, SendSafety, SyncSafety},
};

/// Owned by every scope container so executor storage outlives construction and finalization.
pub(crate) struct Plan {
    pub(crate) compiled: CompiledGraph<TypeId>,
    /// Indexed by registration id.
    pub(crate) executors: Vec<RegistrationExecutor>,
    /// The scope hierarchy, widest first; indexed by `ScopeId`.
    pub(crate) scopes: Vec<ScopeData>,
    /// Provided type of every registration, indexed by registration id.
    type_ids: Vec<TypeId>,
    #[cfg(feature = "async")]
    pub(crate) async_table: AsyncTable,
    #[cfg(feature = "thread_safe")]
    locks: NodeLocks,
    /// Stable storage for both executor tables, including owned runtime fragments.
    /// Declared last so executor tables are dropped first. See `RegistrationExecutor`'s invariants.
    _tree: RcAnyThreadSafety,
}

/// Async construction functions, present when an async container built the plan.
#[cfg(feature = "async")]
pub(crate) use crate::async_impl::AsyncTable;

#[cfg(not(feature = "async"))]
pub(crate) type AsyncTable = ();

/// Cached values by registration id. The vector is allocated on the first write, so entering a
/// scope that caches nothing costs no allocation.
#[derive(Default)]
pub(crate) struct Slots(Vec<Option<RcAnyThreadSafety>>);

impl Slots {
    #[inline]
    pub(crate) fn get(&self, index: usize) -> Option<&RcAnyThreadSafety> {
        self.0.get(index)?.as_ref()
    }

    #[inline]
    pub(crate) fn set(&mut self, index: usize, value: RcAnyThreadSafety, len: usize) {
        if self.0.is_empty() {
            self.0.resize(len, None);
        }
        self.0[index] = Some(value);
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }
}

// Both phases include the implicit container provider.
pub(crate) type ProviderIndex<Tree> = <Node<Tree, ContainerLeaf> as RegistryIndex>::Index;
pub(crate) type Linked<Tree, Links> = <Node<Tree, ContainerLeaf> as Link<ProviderIndex<Tree>, Links>>::Linked;

impl Inner {
    fn close(&self) {
        let resolved = core::mem::take(&mut *self.resolved.write());
        for (index, value) in resolved.into_iter().rev() {
            // An async finalizer needs `close().await` on the async container; like Froodi, it
            // does not run when the container is only dropped.
            if self.plan.compiled.nodes()[index].finalizer == Some(ExecutionKind::Async) {
                continue;
            }
            let executor = &self.plan.executors[index];
            // SAFETY: `value` was constructed by the registration at `index`.
            unsafe { (executor.finalize)(executor.registration, value) };
        }
        self.reset_slots();
        if self.close_parent {
            if let Some(parent) = &self.parent {
                parent.close();
            }
        }
    }

    /// Clears the cache back to the context values.
    pub(crate) fn reset_slots(&self) {
        let mut slots = self.slots.write();
        slots.clear();
        self.plan.fill_from_context(&mut slots, &self.context);
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.close();
    }
}

impl Plan {
    /// A context value of a registered type answers `get` before the factory, as in Froodi.
    fn fill_from_context(&self, slots: &mut Slots, context: &Context) {
        for (type_id, value) in &context.map {
            if let Some(id) = self.compiled.lookup(type_id) {
                slots.set(id.index(), value.clone(), self.executors.len());
            }
        }
    }

    pub(crate) fn build<Tree, Links>(registry: Registry<Tree>) -> Result<RcThreadSafety<Self>, Diagnostics>
    where
        Node<Tree, ContainerLeaf>: RegistryIndex + Link<ProviderIndex<Tree>, Links>,
        Linked<Tree, Links>: CollectExecutors + CollectRegistrations + CollectRuntime + SendSafety + SyncSafety + 'static,
    {
        Self::build_with(registry, |_| AsyncTable::default())
    }

    pub(crate) fn build_with<Tree, Links>(
        registry: Registry<Tree>,
        collect_async_executors: impl FnOnce(&Linked<Tree, Links>) -> AsyncTable,
    ) -> Result<RcThreadSafety<Self>, Diagnostics>
    where
        Node<Tree, ContainerLeaf>: RegistryIndex + Link<ProviderIndex<Tree>, Links>,
        Linked<Tree, Links>: CollectExecutors + CollectRegistrations + CollectRuntime + SendSafety + SyncSafety + 'static,
    {
        let () = <Node<Tree, ContainerLeaf> as Link<ProviderIndex<Tree>, Links>>::VALIDATE;
        let mut scopes = registry.scopes;
        scopes.sort_by_key(|scope| scope.priority);
        scopes.dedup();
        let container = ContainerLeaf {
            scope: *scopes.first().expect("a registry has at least one scope"),
        };
        let mut tree = Node(registry.tree, container).link();
        let mut graph = froodi_compile_core::Graph::new(scopes.iter().copied().map(Into::into).collect());
        tree.collect_registrations(&mut graph.registrations);
        // Collect pointers only after the final allocation. Moving an owning Box after
        // deriving shared raw pointers retags its pointee and invalidates their provenance.
        let tree = RcThreadSafety::new(tree);
        let mut executors = Vec::new();
        tree.collect_executors(&mut executors);
        let mut runtime = Vec::new();
        tree.collect_runtime(&mut runtime);
        for fragment in runtime {
            fragment.describe_runtime(&mut graph.registrations);
            fragment.collect_fragment_executors(&mut executors);
        }
        let type_ids = graph.registrations.iter().map(|registration| registration.key).collect();
        let compiled = compile(graph)?;
        #[cfg(feature = "async")]
        let async_table = {
            let mut table = collect_async_executors(&tree);
            table.fill(executors.len());
            table
        };
        #[cfg(not(feature = "async"))]
        collect_async_executors(&tree);
        Ok(RcThreadSafety::new(Self {
            #[cfg(feature = "async")]
            async_table,
            type_ids,
            #[cfg(feature = "thread_safe")]
            locks: NodeLocks::new(executors.len()),
            compiled,
            _tree: tree,
            executors,
            scopes,
        }))
    }
}

// SAFETY: registration pointers only address the shared tree owned by the same `Plan`, which is
// `Send + Sync` in thread-safe builds; executors hold plain function pointers.
#[cfg(feature = "thread_safe")]
unsafe impl Send for Plan {}
#[cfg(feature = "thread_safe")]
unsafe impl Sync for Plan {}

#[derive(Clone)]
pub struct Container {
    pub(crate) inner: RcThreadSafety<Inner>,
}

pub(crate) struct Inner {
    pub(crate) plan: RcThreadSafety<Plan>,
    pub(crate) level: ScopeId,
    pub(crate) slots: LocalLock<Slots>,
    /// Context values visible in this container: the parent's, overridden by its own.
    pub(crate) context: Context,
    /// Values constructed here whose registration has a finalizer, in construction order.
    pub(crate) resolved: LocalLock<Vec<(usize, RcAnyThreadSafety)>>,
    pub(crate) parent: Option<Container>,
    /// Whether closing this container also closes its parent: set for the levels a builder
    /// created on the way to the requested scope.
    pub(crate) close_parent: bool,
}

impl Container {
    /// Builds the container of a registry.
    ///
    /// # Panics
    /// Panics with the rendered diagnostics if the registry graph is invalid; see [`Self::try_new`].
    #[must_use]
    pub fn new<Tree, Links>(registry: Registry<Tree>) -> Self
    where
        Node<Tree, ContainerLeaf>: RegistryIndex + Link<ProviderIndex<Tree>, Links>,
        Linked<Tree, Links>: CollectExecutors + CollectRegistrations + CollectRuntime + SendSafety + SyncSafety + 'static,
    {
        Self::try_new(registry).unwrap_or_else(|diagnostics| panic!("invalid registry:\n{diagnostics}"))
    }

    /// Builds the container of a registry after compiling its graph.
    ///
    /// # Errors
    /// Returns runtime graph diagnostics, including open-graph cycles and scope violations.
    /// Closed static graphs also check cycles during code generation; missing and ambiguous
    /// static providers fail type checking. See `docs/compile-time/architecture.md` for limits.
    pub fn try_new<Tree, Links>(registry: Registry<Tree>) -> Result<Self, Diagnostics>
    where
        Node<Tree, ContainerLeaf>: RegistryIndex + Link<ProviderIndex<Tree>, Links>,
        Linked<Tree, Links>: CollectExecutors + CollectRegistrations + CollectRuntime + SendSafety + SyncSafety + 'static,
    {
        Ok(Self::root(Plan::build(registry)?, |scope| !scope.is_skipped_by_default))
    }

    /// Like [`Self::new`], but starts in `scope`, which may be skipped by default.
    ///
    /// # Panics
    /// Panics if `scope` is not part of the registry's hierarchy, or on an invalid registry graph.
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new_with_start_scope<Tree, Links, S: Scope>(registry: Registry<Tree>, scope: S) -> Self
    where
        Node<Tree, ContainerLeaf>: RegistryIndex + Link<ProviderIndex<Tree>, Links>,
        Linked<Tree, Links>: CollectExecutors + CollectRegistrations + CollectRuntime + SendSafety + SyncSafety + 'static,
    {
        let priority = scope.priority();
        let plan = Plan::build(registry).unwrap_or_else(|diagnostics| panic!("invalid registry:\n{diagnostics}"));
        Self::root(plan, move |data| data.priority == priority)
    }

    /// Starts at the widest scope and descends until `is_target` accepts a scope, keeping every
    /// level as the parent of the next.
    pub(crate) fn root(plan: RcThreadSafety<Plan>, is_target: impl Fn(&ScopeData) -> bool) -> Self {
        let mut container = Self::with_level(plan, ScopeId(0), None, None, false);
        while !is_target(&container.scope()) {
            let level = ScopeId(container.inner.level.0 + 1);
            assert!(
                level.index() < container.inner.plan.scopes.len(),
                "scope chain ended before reaching a target scope"
            );
            container = Self::with_level(container.inner.plan.clone(), level, Some(container), None, true);
        }
        container
    }

    /// Closes the container: runs the finalizers of the values constructed here since the last
    /// close, newest first, and resets the cache to the context values.
    pub fn close(&self) {
        self.inner.close();
    }

    #[inline]
    #[must_use]
    pub fn enter(self) -> ChildContainerBuilder {
        ChildContainerBuilder { container: self }
    }

    /// Creates a child container in the next scope that is not skipped by default.
    ///
    /// # Errors
    /// See `ChildContainerBuilder::build`.
    #[inline]
    pub fn enter_build(self) -> Result<Container, ScopeErrorKind> {
        self.enter().build()
    }

    /// Descends from `self` one scope at a time until `is_target` accepts a scope, keeping every
    /// level as the parent of the next.
    /// A `context`, when given, is added at every created level.
    fn descend<E>(
        self,
        context: Option<&Context>,
        is_target: impl Fn(&ScopeData) -> bool,
        no_child: E,
        no_target: impl Fn() -> E,
    ) -> Result<Container, E> {
        let plan = self.inner.plan.clone();
        let next = |level: ScopeId| {
            let next = ScopeId(level.0 + 1);
            (next.index() < plan.scopes.len()).then_some(next)
        };
        let mut child = Self::with_level(plan.clone(), next(self.inner.level).ok_or(no_child)?, Some(self), context, false);
        while !is_target(&child.scope()) {
            let level = next(child.inner.level).ok_or_else(&no_target)?;
            child = Self::with_level(plan.clone(), level, Some(child), context, true);
        }
        Ok(child)
    }

    fn with_level(
        plan: RcThreadSafety<Plan>,
        level: ScopeId,
        parent: Option<Container>,
        context: Option<&Context>,
        close_parent: bool,
    ) -> Self {
        let mut visible = parent.as_ref().map(|parent| parent.inner.context.clone()).unwrap_or_default();
        if let Some(context) = context {
            visible.map.extend(context.map.iter().map(|(id, value)| (*id, value.clone())));
        }
        let mut slots = Slots::default();
        plan.fill_from_context(&mut slots, &visible);
        Self {
            inner: RcThreadSafety::new(Inner {
                plan,
                level,
                slots: LocalLock::new(slots),
                context: visible,
                resolved: LocalLock::new(Vec::new()),
                parent,
                close_parent,
            }),
        }
    }

    #[inline]
    pub(crate) fn scope(&self) -> ScopeData {
        self.inner.plan.scopes[self.inner.level.index()]
    }

    /// The ancestor (or `self`) at `level`, which must not be narrower than `self`.
    pub(crate) fn ancestor(&self, level: ScopeId) -> &Container {
        let mut container = self;
        while container.inner.level != level {
            container = container
                .inner
                .parent
                .as_ref()
                .expect("a container has every wider scope as an ancestor");
        }
        container
    }

    /// Gets a scoped dependency from the container.
    ///
    /// # Errors
    /// Returns [`ResolveErrorKind`] if nothing provides `Dep` or its construction fails.
    pub fn get<Dep: SendSafety + SyncSafety + 'static>(&self) -> Result<RcThreadSafety<Dep>, ResolveErrorKind> {
        let type_info = TypeInfo::of::<Dep>();
        let value = match self.inner.plan.compiled.lookup(&TypeId::of::<Dep>()) {
            Some(id) => self.get_at(id.index())?,
            None => match self.inner.context.map.get(&TypeId::of::<Dep>()) {
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

    /// Gets a transient dependency: a new value on every call. The provided-value cache is neither
    /// read nor written.
    ///
    /// # Errors
    /// Returns [`ResolveErrorKind`] if nothing provides `Dep` or its construction fails.
    pub fn get_transient<Dep: 'static>(&self) -> Result<Dep, ResolveErrorKind> {
        let Some(id) = self.inner.plan.compiled.lookup(&TypeId::of::<Dep>()) else {
            return Err(ResolveErrorKind::NoInstantiator {
                type_info: TypeInfo::of::<Dep>(),
            });
        };
        // SAFETY: the registration was found by `Dep`'s `TypeId`, so it provides `Dep`.
        unsafe { self.get_transient_unchecked(id.index()) }
    }

    /// The container a transient value of the registration at `index` is built in: its owning
    /// scope, as in Froodi.
    ///
    /// # Errors
    /// Returns [`ResolveErrorKind::NoAccessible`] if the registration's scope is narrower.
    fn transient_owner(&self, index: usize) -> Result<&Container, ResolveErrorKind> {
        let plan = &*self.inner.plan;
        let scope = plan.compiled.nodes()[index].scope;
        if scope > self.inner.level {
            return Err(ResolveErrorKind::NoAccessible {
                expected_scope_data: plan.scopes[scope.index()],
                actual_scope_data: self.scope(),
            });
        }
        Ok(self.ancestor(scope))
    }

    /// `get_transient` semantics for the registration at `index`, which must provide `Dep`.
    ///
    /// # Errors
    /// Returns [`ResolveErrorKind::IncorrectType`] if it provides another type, or the
    /// construction error.
    pub(crate) fn get_transient_at<Dep: 'static>(&self, index: usize) -> Result<Dep, ResolveErrorKind> {
        let plan = &*self.inner.plan;
        if plan.type_ids[index] != TypeId::of::<Dep>() {
            return Err(ResolveErrorKind::IncorrectType {
                expected: TypeInfo::of::<Dep>(),
                actual: TypeInfo {
                    name: plan.compiled.nodes()[index].type_name,
                    id: plan.type_ids[index],
                },
            });
        }
        // SAFETY: the registration provides `Dep` (checked above).
        unsafe { self.get_transient_unchecked(index) }
    }

    /// `get_transient` semantics for the registration at `index`, without the type check.
    ///
    /// # Safety
    /// The registration at `index` must provide `Dep`.
    pub(crate) unsafe fn get_transient_unchecked<Dep: 'static>(&self, index: usize) -> Result<Dep, ResolveErrorKind> {
        let plan = &*self.inner.plan;
        if let Some(replacement) = plan.compiled.nodes()[index].replaced_by {
            // SAFETY: a replacement provides the same type as the registration it replaces.
            return unsafe { self.get_transient_unchecked(replacement.index()) };
        }
        let owner = self.transient_owner(index)?;
        let executor = &plan.executors[index];
        let mut out = core::mem::MaybeUninit::<Dep>::uninit();
        // SAFETY: guaranteed by the caller; the executor belongs to this plan.
        unsafe {
            (executor.construct_transient)(
                executor.registration,
                owner,
                &plan.compiled.nodes()[index].edges,
                out.as_mut_ptr().cast(),
            )?;
            Ok(out.assume_init())
        }
    }

    pub(crate) fn get_at(&self, index: usize) -> Result<RcAnyThreadSafety, ResolveErrorKind> {
        if let Some(replacement) = self.inner.plan.compiled.nodes()[index].replaced_by {
            return self.get_at(replacement.index());
        }
        if let Some(value) = self.inner.slots.read().get(index) {
            return Ok(value.clone());
        }
        let plan = &*self.inner.plan;
        let node = &plan.compiled.nodes()[index];
        let value = match node.scope.cmp(&self.inner.level) {
            // Owned by a wider scope: resolve there, keep a copy here as Froodi does.
            Ordering::Less => self.ancestor(node.scope).get_at(index)?,
            Ordering::Greater => {
                return Err(ResolveErrorKind::NoAccessible {
                    expected_scope_data: plan.scopes[node.scope.index()],
                    actual_scope_data: self.scope(),
                });
            }
            Ordering::Equal => {
                // Serialize construction of this registration across the tree, then look again:
                // another thread may have cached it meanwhile.
                #[cfg(feature = "thread_safe")]
                let _guard = plan.locks.get(index).lock();
                #[cfg(feature = "thread_safe")]
                if let Some(value) = self.inner.slots.read().get(index) {
                    return Ok(value.clone());
                }
                let executor = &plan.executors[index];
                // SAFETY: this plan owns the registration and its matching erased functions.
                let value = unsafe { (executor.construct)(executor.registration, self, &node.edges) }?;
                if node.finalizer.is_some() {
                    self.inner.resolved.write().push((index, value.clone()));
                }
                if node.cache_provides {
                    self.inner.slots.write().set(index, value.clone(), plan.executors.len());
                }
                return Ok(value);
            }
        };
        if node.cache_provides {
            self.inner.slots.write().set(index, value.clone(), plan.executors.len());
        }
        Ok(value)
    }
}

pub struct ChildContainerBuilder {
    container: Container,
}

impl ChildContainerBuilder {
    #[inline]
    #[must_use]
    pub fn with_context(self, context: Context) -> ChildContainerWithContext {
        ChildContainerWithContext {
            container: self.container,
            context,
        }
    }

    #[inline]
    #[must_use]
    pub fn with_scope<S: Scope>(self, scope: S) -> ChildContainerWithScope<S> {
        ChildContainerWithScope {
            container: self.container,
            scope,
        }
    }

    /// Creates a child container in the next scope that is not skipped by default.
    ///
    /// # Errors
    /// - [`ScopeErrorKind::NoChildRegistries`] if the container is in the narrowest scope.
    /// - [`ScopeErrorKind::NoNonSkippedRegistries`] if every narrower scope is skipped by default.
    pub fn build(self) -> Result<Container, ScopeErrorKind> {
        self.container.descend(
            None,
            |scope| !scope.is_skipped_by_default,
            ScopeErrorKind::NoChildRegistries,
            || ScopeErrorKind::NoNonSkippedRegistries,
        )
    }
}

pub struct ChildContainerWithScope<S> {
    container: Container,
    scope: S,
}

impl<S: Scope> ChildContainerWithScope<S> {
    #[inline]
    #[must_use]
    pub fn with_context(self, context: Context) -> ChildContainerWithScopeAndContext<S> {
        ChildContainerWithScopeAndContext {
            container: self.container,
            scope: self.scope,
            context,
        }
    }

    /// Creates a child container in the given scope, with every scope in between as its parents.
    ///
    /// # Errors
    /// - [`ScopeWithErrorKind::NoChildRegistries`] if the container is in the narrowest scope.
    /// - [`ScopeWithErrorKind::NoChildRegistriesWithScope`] if the scope is not below the container's.
    pub fn build(self) -> Result<Container, ScopeWithErrorKind> {
        let (priority, name) = (self.scope.priority(), self.scope.name());
        self.container.descend(
            None,
            |scope| scope.priority == priority,
            ScopeWithErrorKind::NoChildRegistries,
            || ScopeWithErrorKind::NoChildRegistriesWithScope { name, priority },
        )
    }
}

pub struct ChildContainerWithContext {
    container: Container,
    context: Context,
}

impl ChildContainerWithContext {
    #[inline]
    #[must_use]
    pub fn with_scope<S: Scope>(self, scope: S) -> ChildContainerWithScopeAndContext<S> {
        ChildContainerWithScopeAndContext {
            container: self.container,
            scope,
            context: self.context,
        }
    }

    /// Creates a child container in the next scope that is not skipped by default, with `context`.
    ///
    /// # Errors
    /// See `ChildContainerBuilder::build`.
    pub fn build(self) -> Result<Container, ScopeErrorKind> {
        self.container.descend(
            Some(&self.context),
            |scope| !scope.is_skipped_by_default,
            ScopeErrorKind::NoChildRegistries,
            || ScopeErrorKind::NoNonSkippedRegistries,
        )
    }
}

pub struct ChildContainerWithScopeAndContext<S> {
    container: Container,
    scope: S,
    context: Context,
}

impl<S: Scope> ChildContainerWithScopeAndContext<S> {
    /// Creates a child container in the given scope, with `context`.
    ///
    /// # Errors
    /// See [`ChildContainerWithScope::build`].
    pub fn build(self) -> Result<Container, ScopeWithErrorKind> {
        let (priority, name) = (self.scope.priority(), self.scope.name());
        self.container.descend(
            Some(&self.context),
            |scope| scope.priority == priority,
            ScopeWithErrorKind::NoChildRegistries,
            || ScopeWithErrorKind::NoChildRegistriesWithScope { name, priority },
        )
    }
}
