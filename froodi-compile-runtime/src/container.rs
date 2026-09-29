use alloc::vec::Vec;
use core::any::TypeId;

use core::cmp::Ordering;

use froodi_compile_core::{compile, CompiledGraph, Diagnostics, ScopeId};

use crate::{
    context::Context,
    errors::{ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind, TypeInfo},
    graph::{ContainerLeaf, Describe, Entry, Link, Node, Walk},
    lock::LocalLock,
    registry::Registry,
    scope::{Scope, ScopeData},
    thread_safety::{BoxAnyThreadSafety, RcAnyThreadSafety, RcThreadSafety, SendSafety, SyncSafety},
};

/// State shared by every container of one tree: the compiled graph, the linked registration
/// tree and its construction entries.
struct Plan {
    compiled: CompiledGraph<TypeId>,
    /// Keeps the linked tree alive at a stable address; `root` and the entries point into it.
    _tree: BoxAnyThreadSafety,
    root: *const (),
    /// Indexed by registration id.
    entries: Vec<Entry>,
    /// The scope hierarchy, widest first; indexed by `ScopeId`.
    scopes: Vec<ScopeData>,
}

/// The linked form of a registry tree with the container registration appended.
type Linked<Tree, Links> = <Node<Tree, ContainerLeaf> as Link<Node<Tree, ContainerLeaf>, Links>>::Linked;

impl Inner {
    fn close(&self) {
        let resolved = core::mem::take(&mut *self.resolved.write());
        for (index, value) in resolved.into_iter().rev() {
            let entry = &self.plan.entries[index];
            // SAFETY: `value` was constructed by the registration at `index`.
            unsafe { (entry.finalize)(entry.item, value) };
        }
        {
            let mut slots = self.slots.write();
            slots.iter_mut().for_each(|slot| *slot = None);
            self.plan.fill_from_context(&mut slots, &self.context);
        }
        if self.close_parent {
            if let Some(parent) = &self.parent {
                parent.close();
            }
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.close();
    }
}

impl Plan {
    /// A context value of a registered type answers `get` before the factory, as in Froodi.
    fn fill_from_context(&self, slots: &mut [Option<RcAnyThreadSafety>], context: &Context) {
        for (type_id, value) in &context.map {
            if let Some(id) = self.compiled.lookup(type_id) {
                slots[id.index()] = Some(value.clone());
            }
        }
    }

    /// Compiles the registry graph and links its tree.
    fn build<Tree, Links>(registry: Registry<Tree>) -> Result<RcThreadSafety<Self>, Diagnostics>
    where
        Node<Tree, ContainerLeaf>: Link<Node<Tree, ContainerLeaf>, Links> + Describe,
        Linked<Tree, Links>: Walk<Linked<Tree, Links>> + SendSafety + SyncSafety + 'static,
    {
        let mut scopes = registry.scopes;
        scopes.sort_by_key(|scope| scope.priority);
        scopes.dedup();
        let container = ContainerLeaf {
            scope: *scopes.first().expect("a registry has at least one scope"),
        };
        let registry = Registry::from_parts(Node(registry.tree, container), scopes.clone());
        let compiled = compile(registry.graph())?;
        let tree = alloc::boxed::Box::new(registry.tree.link());
        let mut entries = Vec::new();
        tree.walk(&mut entries);
        let root = core::ptr::from_ref::<Linked<Tree, Links>>(&tree).cast();
        Ok(RcThreadSafety::new(Self {
            compiled,
            _tree: tree,
            root,
            entries,
            scopes,
        }))
    }
}

// SAFETY: the raw pointers only address the boxed tree owned by the same `Plan`, which is
// `Send + Sync` in thread-safe builds; entries hold plain function pointers.
#[cfg(feature = "thread_safe")]
unsafe impl Send for Plan {}
#[cfg(feature = "thread_safe")]
unsafe impl Sync for Plan {}

#[derive(Clone)]
pub struct Container {
    inner: RcThreadSafety<Inner>,
}

struct Inner {
    plan: RcThreadSafety<Plan>,
    /// Position of this container's scope in the hierarchy.
    level: ScopeId,
    /// Cached values by registration id.
    slots: LocalLock<Vec<Option<RcAnyThreadSafety>>>,
    /// Context values visible in this container: the parent's, overridden by its own.
    context: Context,
    /// Values constructed here whose registration has a finalizer, in construction order.
    resolved: LocalLock<Vec<(usize, RcAnyThreadSafety)>>,
    parent: Option<Container>,
    /// Whether closing this container also closes its parent: set for the levels a builder
    /// created on the way to the requested scope.
    close_parent: bool,
}

impl Container {
    /// Builds the container of a registry.
    ///
    /// # Panics
    /// Panics with the rendered diagnostics if the registry graph is invalid; see [`Self::try_new`].
    #[must_use]
    pub fn new<Tree, Links>(registry: Registry<Tree>) -> Self
    where
        Node<Tree, ContainerLeaf>: Link<Node<Tree, ContainerLeaf>, Links> + Describe,
        Linked<Tree, Links>: Walk<Linked<Tree, Links>> + SendSafety + SyncSafety + 'static,
    {
        Self::try_new(registry).unwrap_or_else(|diagnostics| panic!("invalid registry:\n{diagnostics}"))
    }

    /// Builds the container of a registry after compiling its graph.
    ///
    /// # Errors
    /// Returns the graph compiler's diagnostics: duplicates no factory depends on, cycles, scope
    /// violations. Missing and ambiguous providers of static dependencies are compile errors.
    pub fn try_new<Tree, Links>(registry: Registry<Tree>) -> Result<Self, Diagnostics>
    where
        Node<Tree, ContainerLeaf>: Link<Node<Tree, ContainerLeaf>, Links> + Describe,
        Linked<Tree, Links>: Walk<Linked<Tree, Links>> + SendSafety + SyncSafety + 'static,
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
        Node<Tree, ContainerLeaf>: Link<Node<Tree, ContainerLeaf>, Links> + Describe,
        Linked<Tree, Links>: Walk<Linked<Tree, Links>> + SendSafety + SyncSafety + 'static,
    {
        let priority = scope.priority();
        let plan = Plan::build(registry).unwrap_or_else(|diagnostics| panic!("invalid registry:\n{diagnostics}"));
        Self::root(plan, move |data| data.priority == priority)
    }

    /// Starts at the widest scope and descends until `is_target` accepts a scope, keeping every
    /// level as the parent of the next.
    fn root(plan: RcThreadSafety<Plan>, is_target: impl Fn(&ScopeData) -> bool) -> Self {
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

    /// Creates a child container builder.
    #[inline]
    #[must_use]
    pub fn enter(self) -> ChildContainerBuilder {
        ChildContainerBuilder { container: self }
    }

    /// Creates a child container in the next scope that is not skipped by default.
    ///
    /// # Errors
    /// See [`ChildContainerBuilder::build`].
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
        let mut slots = alloc::vec![None; plan.entries.len()];
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
    fn scope(&self) -> ScopeData {
        self.inner.plan.scopes[self.inner.level.index()]
    }

    /// The ancestor (or `self`) at `level`, which must not be narrower than `self`.
    fn ancestor(&self, level: ScopeId) -> &Container {
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
            Some(id) => self.shared(id.index())?,
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
        let plan = &*self.inner.plan;
        let entry = &plan.entries[id.index()];
        let mut out = core::mem::MaybeUninit::<Dep>::uninit();
        // SAFETY: the registration was found by `Dep`'s `TypeId`, so it provides `Dep`, and the
        // entry belongs to the tree `plan.root` points to.
        unsafe {
            (entry.transient)(plan.root, entry.item, self, out.as_mut_ptr().cast())?;
            Ok(out.assume_init())
        }
    }

    /// `get` semantics for the registration at `index`, constructing through its entry.
    fn shared(&self, index: usize) -> Result<RcAnyThreadSafety, ResolveErrorKind> {
        let plan = &*self.inner.plan;
        let entry = &plan.entries[index];
        // SAFETY: the entry was produced by walking the tree `plan.root` points to.
        self.shared_with(index, |container| unsafe { (entry.construct)(plan.root, entry.item, container) })
    }

    /// `get` semantics for the registration at `index`: the cached value, or the result of
    /// `construct`. `construct` must produce the value of that registration; static edges pass a
    /// direct call to its factory.
    pub(crate) fn shared_with(
        &self,
        index: usize,
        construct: impl FnOnce(&Container) -> Result<RcAnyThreadSafety, ResolveErrorKind>,
    ) -> Result<RcAnyThreadSafety, ResolveErrorKind> {
        if let Some(value) = &self.inner.slots.read()[index] {
            return Ok(value.clone());
        }
        let plan = &*self.inner.plan;
        let node = &plan.compiled.nodes()[index];
        let value = match node.scope.cmp(&self.inner.level) {
            // Owned by a wider scope: resolve there, keep a copy here as Froodi does.
            Ordering::Less => self.ancestor(node.scope).shared_with(index, construct)?,
            Ordering::Greater => {
                return Err(ResolveErrorKind::NoAccessible {
                    expected_scope_data: plan.scopes[node.scope.index()],
                    actual_scope_data: self.scope(),
                });
            }
            Ordering::Equal => {
                let value = construct(self)?;
                if node.finalizer.is_some() {
                    self.inner.resolved.write().push((index, value.clone()));
                }
                value
            }
        };
        if node.cache_provides {
            self.inner.slots.write()[index] = Some(value.clone());
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
    /// See [`ChildContainerBuilder::build`].
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
