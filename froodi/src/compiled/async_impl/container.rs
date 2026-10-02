use super::{
    registration::AsyncRegistration,
    registry::{Index, IntoRegistry, Root},
};
use crate::{
    async_impl::{
        container::{ChildContainerBuilder, ChildContainerWithContext, ChildContainerWithScope, ChildContainerWithScopeAndContext},
        Container,
    },
    compiled::{
        container::TypedRegistry,
        linking::{ProviderPath, RegistryIndex},
        Registry,
    },
    utils::thread_safety::{RcThreadSafety, SendSafety, SyncSafety},
    Context, ResolveErrorKind, Scope, ScopeErrorKind, ScopeWithErrorKind,
};
use core::{future::Future, marker::PhantomData};

impl<Out, Inst, Deps, Fin> TypedRegistry for AsyncRegistration<Out, Inst, Deps, Fin> {}

/// Checks requested provider types while using Froodi's async container runtime.
///
/// Import [`TypedContainerExt`] for async `get` and `get_transient`. Typed sync
/// fragments can be extended into this registry. Native fragments and runtime/context
/// provider declarations require [`Container`]. Scope and construction errors remain runtime errors.
pub struct TypedContainer<Providers = ()> {
    inner: Container,
    // Subtyping must not turn a provider proof into one for a different Rust type.
    marker: PhantomData<fn(Providers) -> Providers>,
}

impl TypedContainer {
    /// Builds an async container and retains its inferred provider index.
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

    /// Builds a typed async container at the requested start scope.
    ///
    /// # Panics
    /// Has the same failures as [`Container::new_compiled_with_start_scope`].
    pub fn new_with_start_scope<Tree: TypedRegistry, Links>(
        registry: Registry<Tree>,
        scope: impl Scope + Clone,
    ) -> TypedContainer<Index<Tree>>
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

    pub async fn close(&self) {
        self.inner.close().await;
    }
}

/// Import this trait to check async requests against a typed container's providers.
/// `Path` is inferred; callers write `get::<T>()` or `get_transient::<T>()`.
pub trait TypedContainerExt<Path> {
    #[doc(hidden)]
    type Providers;

    fn get<Dep: SendSafety + SyncSafety + 'static>(
        &self,
    ) -> impl Future<Output = Result<RcThreadSafety<Dep>, ResolveErrorKind>> + SendSafety + '_
    where
        Self::Providers: ProviderPath<Dep, Path>;

    fn get_transient<Dep: 'static>(&self) -> impl Future<Output = Result<Dep, ResolveErrorKind>> + SendSafety + '_
    where
        Self::Providers: ProviderPath<Dep, Path>;
}

impl<Providers, Path> TypedContainerExt<Path> for TypedContainer<Providers> {
    type Providers = Providers;

    fn get<Dep: SendSafety + SyncSafety + 'static>(
        &self,
    ) -> impl Future<Output = Result<RcThreadSafety<Dep>, ResolveErrorKind>> + SendSafety + '_
    where
        Providers: ProviderPath<Dep, Path>,
    {
        self.inner.get::<Dep>()
    }

    fn get_transient<Dep: 'static>(&self) -> impl Future<Output = Result<Dep, ResolveErrorKind>> + SendSafety + '_
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
