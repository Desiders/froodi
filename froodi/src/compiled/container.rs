use super::{
    linking::{Empty, Node, ProviderPath, RegistryIndex},
    registration::{LocatedRegistration, Registration},
    registry::{Index, IntoRegistry, Registry, Root},
};
use crate::{
    container::{ChildContainerBuilder, ChildContainerWithContext, ChildContainerWithScope, ChildContainerWithScopeAndContext},
    utils::thread_safety::{RcThreadSafety, SendSafety, SyncSafety},
    Container, Context, ResolveErrorKind, Scope, ScopeErrorKind, ScopeWithErrorKind,
};
use core::marker::PhantomData;

// Kept private to this module tree: external registry implementations cannot claim this proof.
#[diagnostic::on_unimplemented(
    message = "a typed container requires concrete registrations in typed fragments",
    note = "use Container::new for native fragments or runtime/context provider declarations"
)]
pub trait TypedRegistry {}

impl TypedRegistry for Empty {}

impl<Left: TypedRegistry, Right: TypedRegistry> TypedRegistry for Node<Left, Right> {}

impl<Out, Inst, Deps, Fin> TypedRegistry for Registration<Out, Inst, Deps, Fin> {}

impl<Reg: TypedRegistry, Source> TypedRegistry for LocatedRegistration<Reg, Source> {}

/// Checks requested provider types while using the existing container runtime.
///
/// Import [`TypedContainerExt`] for `get` and `get_transient`. Scope accessibility,
/// instantiation errors and cache behavior remain those of [`Container`].
/// Native fragments and runtime/context provider declarations require `Container`.
///
/// ```
/// use froodi::{compiled_registry, instance, TypedContainer, TypedContainerExt as _, DefaultScope::App};
///
/// let container = TypedContainer::new(compiled_registry! { provide(App, instance(7u32)) });
/// assert_eq!(*container.get::<u32>().unwrap(), 7);
/// ```
pub struct TypedContainer<Providers = ()> {
    inner: Container,
    // Subtyping must not turn a provider proof into one for a different Rust type.
    marker: PhantomData<fn(Providers) -> Providers>,
}

impl TypedContainer {
    /// Builds a container and retains its inferred provider index.
    ///
    /// # Panics
    /// Has the same construction and validation failures as [`Container::new`].
    pub fn new<Tree: TypedRegistry, Links>(registry: Registry<Tree>) -> TypedContainer<Index<Tree>>
    where
        Root<Tree>: RegistryIndex,
        Registry<Tree>: IntoRegistry<Links>,
    {
        TypedContainer::from_inner(Container::new(registry))
    }

    /// Builds a typed container at the requested start scope.
    ///
    /// # Panics
    /// Has the same failures as [`Container::new_compiled_with_start_scope`].
    pub fn new_with_start_scope<Tree: TypedRegistry, Links>(registry: Registry<Tree>, scope: impl Scope) -> TypedContainer<Index<Tree>>
    where
        Root<Tree>: RegistryIndex,
        Registry<Tree>: IntoRegistry<Links>,
    {
        TypedContainer::from_inner(Container::new_compiled_with_start_scope(registry, scope))
    }
}

impl<Providers> Clone for TypedContainer<Providers> {
    fn clone(&self) -> Self {
        Self::from_inner(self.inner.clone())
    }
}

impl<Providers> TypedContainer<Providers> {
    fn from_inner(inner: Container) -> Self {
        Self {
            inner,
            marker: PhantomData,
        }
    }

    /// Discards provider proofs for use with dynamic lookups and integrations.
    #[must_use]
    pub fn into_container(self) -> Container {
        self.inner
    }

    #[must_use]
    pub fn enter(self) -> TypedChildContainerBuilder<Providers, ChildContainerBuilder> {
        TypedChildContainerBuilder::new(self.inner.enter())
    }

    /// # Errors
    /// Returns the native scope error when no child scope is available.
    pub fn enter_build(self) -> Result<Self, ScopeErrorKind> {
        self.inner.enter_build().map(Self::from_inner)
    }

    pub fn close(&self) {
        self.inner.close();
    }
}

/// Import this trait to check requested types against a typed container's providers.
/// `Path` is inferred; callers write `get::<T>()` or `get_transient::<T>()`.
pub trait TypedContainerExt<Path> {
    #[doc(hidden)]
    type Providers;

    /// # Errors
    /// Returns the native resolution error for inaccessible scopes or failed construction.
    fn get<Dep: SendSafety + SyncSafety + 'static>(&self) -> Result<RcThreadSafety<Dep>, ResolveErrorKind>
    where
        Self::Providers: ProviderPath<Dep, Path>;

    /// # Errors
    /// Returns the native resolution error for inaccessible scopes or failed construction.
    fn get_transient<Dep: 'static>(&self) -> Result<Dep, ResolveErrorKind>
    where
        Self::Providers: ProviderPath<Dep, Path>;
}

impl<Providers, Path> TypedContainerExt<Path> for TypedContainer<Providers> {
    type Providers = Providers;

    fn get<Dep: SendSafety + SyncSafety + 'static>(&self) -> Result<RcThreadSafety<Dep>, ResolveErrorKind>
    where
        Providers: ProviderPath<Dep, Path>,
    {
        self.inner.get::<Dep>()
    }

    fn get_transient<Dep: 'static>(&self) -> Result<Dep, ResolveErrorKind>
    where
        Providers: ProviderPath<Dep, Path>,
    {
        self.inner.get_transient::<Dep>()
    }
}

pub struct TypedChildContainerBuilder<Providers, Builder> {
    inner: Builder,
    marker: PhantomData<fn(Providers) -> Providers>,
}

impl<Providers, Builder> TypedChildContainerBuilder<Providers, Builder> {
    fn new(inner: Builder) -> Self {
        Self {
            inner,
            marker: PhantomData,
        }
    }
}

impl<Providers> TypedChildContainerBuilder<Providers, ChildContainerBuilder> {
    #[must_use]
    pub fn with_scope<S: Scope>(self, scope: S) -> TypedChildContainerBuilder<Providers, ChildContainerWithScope<S>> {
        TypedChildContainerBuilder::new(self.inner.with_scope(scope))
    }

    #[must_use]
    pub fn with_context(self, context: Context) -> TypedChildContainerBuilder<Providers, ChildContainerWithContext> {
        TypedChildContainerBuilder::new(self.inner.with_context(context))
    }

    pub fn build(self) -> Result<TypedContainer<Providers>, ScopeErrorKind> {
        self.inner.build().map(TypedContainer::from_inner)
    }
}

impl<Providers, S: Scope> TypedChildContainerBuilder<Providers, ChildContainerWithScope<S>> {
    #[must_use]
    pub fn with_context(self, context: Context) -> TypedChildContainerBuilder<Providers, ChildContainerWithScopeAndContext<S>> {
        TypedChildContainerBuilder::new(self.inner.with_context(context))
    }

    pub fn build(self) -> Result<TypedContainer<Providers>, ScopeWithErrorKind> {
        self.inner.build().map(TypedContainer::from_inner)
    }
}

impl<Providers> TypedChildContainerBuilder<Providers, ChildContainerWithContext> {
    #[must_use]
    pub fn with_scope<S: Scope>(self, scope: S) -> TypedChildContainerBuilder<Providers, ChildContainerWithScopeAndContext<S>> {
        TypedChildContainerBuilder::new(self.inner.with_scope(scope))
    }

    pub fn build(self) -> Result<TypedContainer<Providers>, ScopeErrorKind> {
        self.inner.build().map(TypedContainer::from_inner)
    }
}

impl<Providers, S: Scope> TypedChildContainerBuilder<Providers, ChildContainerWithScopeAndContext<S>> {
    pub fn build(self) -> Result<TypedContainer<Providers>, ScopeWithErrorKind> {
        self.inner.build().map(TypedContainer::from_inner)
    }
}
