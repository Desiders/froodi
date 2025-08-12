use alloc::{boxed::Box, sync::Arc};
use async_lock::Mutex;
use async_recursion::async_recursion;
use core::any::{type_name, TypeId};
use parking_lot::Mutex as SyncMutex;
use tracing::{debug, debug_span, error};

use super::{
    registry::{InstantiatorInnerData, RegistriesBuilder, Registry},
    service::Service as _,
};
use crate::{
    cache::{Cache, Resolved},
    container::{BoxedContainerInner as LifetimedSyncContainerInner, Container as SyncContainer, ContainerInner as SyncContainerInner},
    context::Context,
    errors::{ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind},
    registry::Registry as SyncRegistry,
    scope::Scope,
    InstantiatorErrorKind,
};

#[derive(Clone)]
pub struct Container {
    inner: Arc<Mutex<ContainerInner>>,
    sync: SyncContainer,
}

impl Container {
    /// Creates container and builds it with next non-skipped scope.
    /// For example, in case of [`crate::scope::DefaultScope`], [`crate::scope::DefaultScope::Runtime`] will be skipped to [`crate::scope::DefaultScope::App`],
    /// because the first flagged as skippable, but it will be in container as parent of current.
    ///
    /// # Warning
    /// This method skips first skippable scopes, if you want to use one of them, use [`Self::new_with_start_scope`].
    ///
    /// # Panics
    /// - Panics if registries builder doesn't create any registry.
    ///   This can occur if scopes are empty.
    /// - Panics if there are no child registries.
    ///   This can occur if count of scopes is 1.
    /// - Panics if all scopes except the first one are skipped by default.
    #[inline]
    #[must_use]
    pub fn new<S: Scope + Clone>(registries_builder: RegistriesBuilder<S>) -> Self {
        let (registries, sync_registries) = registries_builder.build();
        let mut registries = registries.into_iter();
        let (root_registry, child_registries) = if let Some(root_registry) = registries.next() {
            (Arc::new(root_registry), registries.map(Arc::new).collect())
        } else {
            panic!("registries len (is 0) should be > 1");
        };
        let mut sync_registries = sync_registries.into_iter();
        let (root_sync_registry, child_sync_registries) = if let Some(root_registry) = sync_registries.next() {
            (Arc::new(root_registry), sync_registries.map(Arc::new).collect())
        } else {
            panic!("registries len (is 0) should be > 1");
        };

        let mut sync_container = LifetimedSyncContainerInner {
            cache: Cache::new(),
            context: Context::new(),
            root_registry: root_sync_registry,
            child_registries: child_sync_registries,
            parent: None,
            close_parent: false,
        };
        let mut container = BoxedContainerInner {
            cache: Cache::new(),
            context: Context::new(),
            root_registry,
            child_registries,
            parent: None,
            close_parent: false,
            sync_container: sync_container.clone(),
        };

        let mut iter = container.child_registries.iter();
        let mut registry = (*iter.next().expect("registries len (is 1) should be > 1")).clone();
        let mut child_registries = iter.cloned().collect();

        let mut sync_iter = sync_container.child_registries.iter();
        let mut sync_registry = (*sync_iter.next().expect("registries len (is 1) should be > 1")).clone();
        let mut child_sync_registries = sync_iter.cloned().collect();

        let mut search_next = container.root_registry.scope.is_skipped_by_default;
        while search_next {
            sync_container = sync_container.init_child(sync_registry, child_sync_registries, true);
            container = container.init_child(registry, child_registries, true, sync_container.clone());

            search_next = sync_container.root_registry.scope.is_skipped_by_default;
            if search_next {
                let mut iter = container.child_registries.iter();
                registry = (*iter.next().expect("last scope can't be skipped by default")).clone();
                child_registries = iter.cloned().collect();

                let mut sync_iter = sync_container.child_registries.iter();
                sync_registry = (*sync_iter.next().expect("last scope can't be skipped by default")).clone();
                child_sync_registries = sync_iter.cloned().collect();
            } else {
                break;
            }
        }

        container.into()
    }

    /// Creates container with start scope
    /// # Panics
    /// - Panics if registries builder doesn't create any registry.
    ///   This can occur if scopes are empty.
    /// - Panics if specified start scope not found in scopes.
    #[inline]
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new_with_start_scope<S: Scope + Clone>(registries_builder: RegistriesBuilder<S>, scope: S) -> Self {
        let (registries, sync_registries) = registries_builder.build();
        let mut registries = registries.into_iter();
        let (root_registry, child_registries) = if let Some(root_registry) = registries.next() {
            (Arc::new(root_registry), registries.map(Arc::new).collect())
        } else {
            panic!("registries len (is 0) should be > 1");
        };
        let mut sync_registries = sync_registries.into_iter();
        let (root_sync_registry, child_sync_registries) = if let Some(root_registry) = sync_registries.next() {
            (Arc::new(root_registry), sync_registries.map(Arc::new).collect())
        } else {
            panic!("registries len (is 0) should be > 1");
        };

        let mut container_priority = root_registry.scope.priority;
        let priority = scope.priority();

        let mut sync_container = LifetimedSyncContainerInner {
            cache: Cache::new(),
            context: Context::new(),
            root_registry: root_sync_registry,
            child_registries: child_sync_registries,
            parent: None,
            close_parent: false,
        };
        let mut container = BoxedContainerInner {
            cache: Cache::new(),
            context: Context::new(),
            root_registry,
            child_registries,
            parent: None,
            close_parent: false,
            sync_container: sync_container.clone(),
        };

        if container_priority == priority {
            return container.into();
        }

        let mut iter = container.child_registries.iter();
        let mut registry = (*iter.next().expect("last scope can't be with another priority")).clone();
        let mut child_registries = iter.cloned().collect();

        let mut sync_iter = sync_container.child_registries.iter();
        let mut sync_registry = (*sync_iter.next().expect("last scope can't be with another priority")).clone();
        let mut child_sync_registries = sync_iter.cloned().collect();

        let mut search_next = container_priority != priority;
        while search_next {
            sync_container = sync_container.init_child(sync_registry, child_sync_registries, true);
            container = container.init_child(registry, child_registries, true, sync_container.clone());

            container_priority = container.root_registry.scope.priority;
            search_next = container_priority != priority;
            if search_next {
                let mut iter = container.child_registries.iter();
                registry = (*iter.next().expect("last scope can't be with another priority")).clone();
                child_registries = iter.cloned().collect();

                let mut sync_iter = sync_container.child_registries.iter();
                sync_registry = (*sync_iter.next().expect("last scope can't be with another priority")).clone();
                child_sync_registries = sync_iter.cloned().collect();
            } else {
                break;
            }
        }

        container.into()
    }

    /// Creates child container builder
    #[inline]
    #[must_use]
    pub fn enter(self) -> ChildContainerBuiler {
        ChildContainerBuiler { container: self }
    }

    /// Creates child container and builds it with next non-skipped scope
    /// For example, in case of [`crate::scope::DefaultScope`], [`crate::scope::DefaultScope::Runtime`] will be skipped to [`crate::scope::DefaultScope::App`],
    /// because the first flagged as skippable, but it will be in container as parent of current.
    ///
    /// # Warning
    /// This method skips skippable scopes, if you want to use one of them, use [`ChildContainerBuiler::with_scope`].
    ///
    /// # Errors
    /// - Returns [`ScopeErrorKind::NoChildRegistries`] if there are no registries
    /// - Returns [`ScopeErrorKind::NoNonSkippedRegistries`] if there are no non-skipped registries
    #[inline]
    pub async fn enter_build(self) -> Result<Container, ScopeErrorKind> {
        self.enter().build().await
    }

    /// Gets a scoped dependency from the container
    ///
    /// # Notes
    /// This method resolves a dependency from the container,
    /// so it should be used for dependencies that are cached or shared,
    /// and with optional finalizer.
    #[allow(clippy::missing_errors_doc, clippy::multiple_bound_locations)]
    #[async_recursion]
    pub async fn get<Dep: Send + Sync + 'static>(&self) -> Result<Arc<Dep>, ResolveErrorKind> {
        let span = debug_span!("resolve", dependency = type_name::<Dep>());
        let _guard = span.enter();

        let type_id = TypeId::of::<Dep>();

        if let Some(dependency) = self.inner.lock().await.cache.get(&type_id) {
            debug!("Found in cache");
            return Ok(dependency);
        }
        debug!("Not found in cache");

        let guard = self.inner.lock().await;
        let Some(InstantiatorInnerData {
            mut instantiator,
            finalizer,
            config,
        }) = guard.root_registry.get_instantiator_data(&type_id)
        else {
            if let Some(parent) = &guard.parent {
                debug!("No instantiator found, trying parent container");
                return match parent.get::<Dep>().await {
                    Ok(dependency) => {
                        drop(guard);
                        let mut guard = self.inner.lock().await;
                        guard.cache.insert_rc(dependency.clone());
                        Ok(dependency)
                    }
                    Err(_err) => {
                        debug!("No instantiator found, trying sync container");
                        return self.sync.get();
                    }
                };
            }
            drop(guard);

            debug!("No instantiator found, trying sync container");
            return self.sync.get();
        };
        drop(guard);

        match instantiator.call(self.clone()).await {
            Ok(dependency) => match dependency.downcast::<Dep>() {
                Ok(dependency) => {
                    let dependency = Arc::new(*dependency);
                    let mut guard = self.inner.lock().await;
                    if config.cache_provides {
                        guard.cache.insert_rc(dependency.clone());
                        debug!("Cached");
                    }
                    if finalizer.is_some() {
                        guard.cache.push_resolved(Resolved {
                            type_id,
                            dependency: dependency.clone(),
                        });
                        debug!("Pushed to resolved set");
                    }
                    drop(guard);
                    Ok(dependency)
                }
                Err(incorrect_type) => {
                    let err = ResolveErrorKind::IncorrectType {
                        expected: type_id,
                        actual: (*incorrect_type).type_id(),
                    };
                    error!("{}", err);
                    Err(err)
                }
            },
            Err(InstantiatorErrorKind::Deps(err)) => {
                error!("{}", err);
                Err(ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(Box::new(err))))
            }
            Err(InstantiatorErrorKind::Factory(err)) => {
                error!("{}", err);
                Err(ResolveErrorKind::Instantiator(InstantiatorErrorKind::Factory(err)))
            }
        }
    }

    /// Gets a transient dependency from the container
    ///
    /// # Notes
    /// This method resolves a new instance of the dependency each time it is called,
    /// so it should be used for dependencies that are not cached or shared, and without finalizer.
    #[allow(clippy::missing_errors_doc, clippy::multiple_bound_locations)]
    #[async_recursion]
    pub async fn get_transient<Dep: 'static>(&self) -> Result<Dep, ResolveErrorKind> {
        let span = debug_span!("resolve", dependency = type_name::<Dep>());
        let _guard = span.enter();

        let type_id = TypeId::of::<Dep>();

        let guard = self.inner.lock().await;
        let Some(mut instantiator) = guard.root_registry.get_instantiator(&type_id) else {
            if let Some(parent) = &guard.parent {
                debug!("No instantiator found, trying parent container");
                return match parent.get_transient().await {
                    Ok(dependency) => Ok(dependency),
                    Err(_err) => {
                        debug!("No instantiator found, trying sync container");
                        self.sync.get_transient()
                    }
                };
            }
            drop(guard);

            debug!("No instantiator found, trying sync container");
            return self.sync.get_transient();
        };
        drop(guard);

        match instantiator.call(self.clone()).await {
            Ok(dependency) => match dependency.downcast::<Dep>() {
                Ok(dependency) => Ok(*dependency),
                Err(incorrect_type) => {
                    let err = ResolveErrorKind::IncorrectType {
                        expected: type_id,
                        actual: (*incorrect_type).type_id(),
                    };
                    error!("{}", err);
                    Err(err)
                }
            },
            Err(InstantiatorErrorKind::Deps(err)) => {
                error!("{}", err);
                Err(ResolveErrorKind::Instantiator(InstantiatorErrorKind::Deps(Box::new(err))))
            }
            Err(InstantiatorErrorKind::Factory(err)) => {
                error!("{}", err);
                Err(ResolveErrorKind::Instantiator(InstantiatorErrorKind::Factory(err)))
            }
        }
    }

    /// Closes the container, calling finalizers for resolved dependencies in LIFO order.
    ///
    /// # Warning
    /// This method can be called multiple times, but it will only call finalizers for dependencies that were resolved since the last call
    #[async_recursion]
    pub async fn close(&self) {
        let mut guard = self.inner.lock().await;
        let close_parent = guard.close_parent;

        guard.close_with_parent_flag(false).await;

        {
            let mut sync_guard = self.sync.inner.lock();
            sync_guard.close_with_parent_flag(false);
        }

        if close_parent {
            if let Some(parent) = &mut guard.parent {
                parent.close().await;
            }
        }
    }
}

impl Container {
    #[inline]
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    async fn init_child_with_context(
        self,
        sync_container: SyncContainer,
        context: Context,
        root_registry: Arc<Registry>,
        child_registries: Box<[Arc<Registry>]>,
        root_sync_registry: Arc<SyncRegistry>,
        child_sync_registries: Box<[Arc<SyncRegistry>]>,
        close_parent: bool,
    ) -> Self {
        let inner = self.inner.lock().await;
        let mut cache = inner.cache.child();
        cache.append_context(&context);
        drop(inner);

        let sync_inner = self.sync.inner.lock();
        let mut sync_cache = sync_inner.cache.child();
        sync_cache.append_context(&context);
        drop(sync_inner);

        Self {
            inner: Arc::new(Mutex::new(ContainerInner {
                cache,
                context: context.clone(),
                root_registry,
                child_registries,
                parent: Some(self),
                close_parent,
            })),
            sync: SyncContainer {
                inner: Arc::new(SyncMutex::new(SyncContainerInner {
                    cache: sync_cache,
                    context,
                    root_registry: root_sync_registry,
                    child_registries: child_sync_registries,
                    parent: Some(sync_container),
                    close_parent,
                })),
            },
        }
    }

    #[inline]
    #[must_use]
    async fn init_child(
        self,
        sync_container: SyncContainer,
        root_registry: Arc<Registry>,
        child_registries: Box<[Arc<Registry>]>,
        root_sync_registry: Arc<SyncRegistry>,
        child_sync_registries: Box<[Arc<SyncRegistry>]>,
        close_parent: bool,
    ) -> Self {
        let inner = self.inner.lock().await;
        let mut cache = inner.cache.child();
        let context = inner.context.clone();
        cache.append_context(&context);
        drop(inner);

        let sync_inner = self.sync.inner.lock();
        let mut sync_cache = sync_inner.cache.child();
        let sync_context = sync_inner.context.clone();
        sync_cache.append_context(&sync_context);
        drop(sync_inner);

        Self {
            inner: Arc::new(Mutex::new(ContainerInner {
                cache,
                context,
                root_registry,
                child_registries,
                parent: Some(self),
                close_parent,
            })),
            sync: SyncContainer {
                inner: Arc::new(SyncMutex::new(SyncContainerInner {
                    cache: sync_cache,
                    context: sync_context,
                    root_registry: root_sync_registry,
                    child_registries: child_sync_registries,
                    parent: Some(sync_container),
                    close_parent,
                })),
            },
        }
    }
}

pub struct ChildContainerBuiler {
    container: Container,
}

impl ChildContainerBuiler {
    #[inline]
    #[must_use]
    pub fn with_scope<S: Scope>(self, scope: S) -> ChildContainerWithScope<S> {
        ChildContainerWithScope {
            container: self.container,
            scope,
        }
    }

    #[inline]
    #[must_use]
    pub fn with_context(self, context: Context) -> ChildContainerWithContext {
        ChildContainerWithContext {
            container: self.container,
            context,
        }
    }

    /// Creates child container with next non-skipped scope.
    /// For example, in case of [`crate::scope::DefaultScope`], [`crate::scope::DefaultScope::Runtime`] will be skipped to [`crate::scope::DefaultScope::App`],
    /// because the first flagged as skippable, but it will be in container as parent of current.
    ///
    /// # Errors
    /// - Returns [`ScopeErrorKind::NoChildRegistries`] if there are no registries
    /// - Returns [`ScopeErrorKind::NoNonSkippedRegistries`] if there are no non-skipped registries
    ///
    /// # Warning
    /// This method skips first children skippable scopes, if you want to use one of them, use [`ChildContainerBuiler::with_scope`].
    pub async fn build(self) -> Result<Container, ScopeErrorKind> {
        use ScopeErrorKind::{NoChildRegistries, NoNonSkippedRegistries};

        let inner = self.container.inner.lock().await;
        let mut iter = inner.child_registries.iter();
        let registry = (*iter.next().ok_or(NoChildRegistries)?).clone();
        let child_registries = iter.cloned().collect();
        drop(inner);

        let sync_inner = self.container.sync.inner.lock();
        let mut sync_iter = sync_inner.child_registries.iter();
        let sync_registry = (*sync_iter.next().ok_or(NoChildRegistries)?).clone();
        let child_sync_registries = sync_iter.cloned().collect();
        drop(sync_inner);

        let sync_container = self.container.sync.clone();
        let mut child = self
            .container
            .init_child(
                sync_container,
                registry,
                child_registries,
                sync_registry,
                child_sync_registries,
                false,
            )
            .await;
        let mut inner = child.inner.lock().await;
        let mut sync_inner = child.sync.inner.lock();
        while inner.root_registry.scope.is_skipped_by_default {
            let mut iter = inner.child_registries.iter();
            let registry = (*iter.next().ok_or(NoNonSkippedRegistries)?).clone();
            let child_registries = iter.cloned().collect();
            drop(inner);

            let mut sync_iter = sync_inner.child_registries.iter();
            let sync_registry = (*sync_iter.next().ok_or(NoNonSkippedRegistries)?).clone();
            let child_sync_registries = sync_iter.cloned().collect();
            drop(sync_inner);

            let sync_container = child.sync.clone();
            child = child
                .init_child(
                    sync_container,
                    registry,
                    child_registries,
                    sync_registry,
                    child_sync_registries,
                    true,
                )
                .await;
            inner = child.inner.lock().await;
            sync_inner = child.sync.inner.lock();
        }
        drop(inner);
        drop(sync_inner);

        Ok(child)
    }
}

pub struct ChildContainerWithScope<S> {
    container: Container,
    scope: S,
}

impl<S> ChildContainerWithScope<S>
where
    S: Scope,
{
    #[inline]
    #[must_use]
    pub fn with_context(self, context: Context) -> ChildContainerWithScopeAndContext<S> {
        ChildContainerWithScopeAndContext {
            container: self.container,
            scope: self.scope,
            context,
        }
    }

    /// Creates child container with specified scope.
    ///
    /// # Errors
    /// - Returns [`ScopeWithErrorKind::NoChildRegistries`] if there are no registries
    /// - Returns [`ScopeWithErrorKind::NoChildRegistriesWithScope`] if there are no registries with specified scope
    ///
    /// # Warning
    /// If you want just to use next non-skipped scope, use [`ChildContainerBuiler::with_scope`]
    pub async fn build(self) -> Result<Container, ScopeWithErrorKind> {
        use ScopeWithErrorKind::{NoChildRegistries, NoChildRegistriesWithScope};

        let priority = self.scope.priority();

        let inner = self.container.inner.lock().await;
        let mut iter = inner.child_registries.iter();
        let registry = (*iter.next().ok_or(NoChildRegistries)?).clone();
        let child_registries = iter.cloned().collect();
        drop(inner);

        let sync_inner = self.container.sync.inner.lock();
        let mut sync_iter = sync_inner.child_registries.iter();
        let sync_registry = (*sync_iter.next().ok_or(NoChildRegistries)?).clone();
        let child_sync_registries = sync_iter.cloned().collect();
        drop(sync_inner);

        let sync_container = self.container.sync.clone();
        let mut child = self
            .container
            .init_child(
                sync_container,
                registry,
                child_registries,
                sync_registry,
                child_sync_registries,
                false,
            )
            .await;
        let mut inner = child.inner.lock().await;
        let mut sync_inner = child.sync.inner.lock();
        while inner.root_registry.scope.priority != priority {
            let mut iter = inner.child_registries.iter();
            let registry = (*iter.next().ok_or(NoChildRegistriesWithScope {
                name: self.scope.name(),
                priority,
            })?)
            .clone();
            let child_registries = iter.cloned().collect();
            drop(inner);

            let mut sync_iter = sync_inner.child_registries.iter();
            let sync_registry = (*sync_iter.next().ok_or(NoChildRegistriesWithScope {
                name: self.scope.name(),
                priority,
            })?)
            .clone();
            let child_sync_registries = sync_iter.cloned().collect();
            drop(sync_inner);

            let sync_container = child.sync.clone();
            child = child
                .init_child(
                    sync_container,
                    registry,
                    child_registries,
                    sync_registry,
                    child_sync_registries,
                    true,
                )
                .await;
            inner = child.inner.lock().await;
            sync_inner = child.sync.inner.lock();
        }
        drop(inner);
        drop(sync_inner);

        Ok(child)
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

    /// Creates child container with next non-skipped scope and passes context to it.
    ///
    /// # Errors
    /// - Returns [`ScopeErrorKind::NoChildRegistries`] if there are no registries
    /// - Returns [`ScopeErrorKind::NoNonSkippedRegistries`] if there are no non-skipped registries
    ///
    /// # Warning
    /// This method skips first children skippable scopes, if you want to use one of them, use [`ChildContainerBuiler::with_scope`]
    pub async fn build(self) -> Result<Container, ScopeErrorKind> {
        use ScopeErrorKind::{NoChildRegistries, NoNonSkippedRegistries};

        let inner = self.container.inner.lock().await;
        let mut iter = inner.child_registries.iter();
        let registry = (*iter.next().ok_or(NoChildRegistries)?).clone();
        let child_registries = iter.cloned().collect();
        drop(inner);

        let sync_inner = self.container.sync.inner.lock();
        let mut sync_iter = sync_inner.child_registries.iter();
        let sync_registry = (*sync_iter.next().ok_or(NoChildRegistries)?).clone();
        let child_sync_registries = sync_iter.cloned().collect();
        drop(sync_inner);

        let sync_container = self.container.sync.clone();
        let mut child = self
            .container
            .init_child_with_context(
                sync_container,
                self.context.clone(),
                registry,
                child_registries,
                sync_registry,
                child_sync_registries,
                false,
            )
            .await;
        let mut inner = child.inner.lock().await;
        let mut sync_inner = child.sync.inner.lock();
        while inner.root_registry.scope.is_skipped_by_default {
            let mut iter = inner.child_registries.iter();
            let registry = (*iter.next().ok_or(NoNonSkippedRegistries)?).clone();
            let child_registries = iter.cloned().collect();
            drop(inner);

            let mut sync_iter = sync_inner.child_registries.iter();
            let sync_registry = (*sync_iter.next().ok_or(NoNonSkippedRegistries)?).clone();
            let child_sync_registries = sync_iter.cloned().collect();
            drop(sync_inner);

            let sync_container = child.sync.clone();
            child = child
                .init_child_with_context(
                    sync_container,
                    self.context.clone(),
                    registry,
                    child_registries,
                    sync_registry,
                    child_sync_registries,
                    true,
                )
                .await;
            inner = child.inner.lock().await;
            sync_inner = child.sync.inner.lock();
        }
        drop(inner);
        drop(sync_inner);

        Ok(child)
    }
}

pub struct ChildContainerWithScopeAndContext<S> {
    container: Container,
    scope: S,
    context: Context,
}

impl<S> ChildContainerWithScopeAndContext<S>
where
    S: Scope,
{
    /// Creates child container with specified scope and passes context to it.
    ///
    /// # Errors
    /// - Returns [`ScopeWithErrorKind::NoChildRegistries`] if there are no registries
    /// - Returns [`ScopeWithErrorKind::NoChildRegistriesWithScope`] if there are no registries with specified scope
    ///
    /// # Warning
    /// If you want just to use next non-skipped scope, use [`ChildContainerBuiler::with_scope`]
    pub async fn build(self) -> Result<Container, ScopeWithErrorKind> {
        use ScopeWithErrorKind::{NoChildRegistries, NoChildRegistriesWithScope};

        let priority = self.scope.priority();

        let inner = self.container.inner.lock().await;
        let mut iter = inner.child_registries.iter();
        let registry = (*iter.next().ok_or(NoChildRegistries)?).clone();
        let child_registries = iter.cloned().collect();
        drop(inner);

        let sync_inner = self.container.sync.inner.lock();
        let mut sync_iter = sync_inner.child_registries.iter();
        let sync_registry = (*sync_iter.next().ok_or(NoChildRegistries)?).clone();
        let child_sync_registries = sync_iter.cloned().collect();
        drop(sync_inner);

        let sync_container = self.container.sync.clone();
        let mut child = self
            .container
            .init_child_with_context(
                sync_container,
                self.context.clone(),
                registry,
                child_registries,
                sync_registry,
                child_sync_registries,
                false,
            )
            .await;
        let mut inner = child.inner.lock().await;
        let mut sync_inner = child.sync.inner.lock();
        while inner.root_registry.scope.priority != priority {
            let mut iter = inner.child_registries.iter();
            let registry = (*iter.next().ok_or(NoChildRegistriesWithScope {
                name: self.scope.name(),
                priority,
            })?)
            .clone();
            let child_registries = iter.cloned().collect();
            drop(inner);

            let mut sync_iter = sync_inner.child_registries.iter();
            let sync_registry = (*sync_iter.next().ok_or(NoChildRegistriesWithScope {
                name: self.scope.name(),
                priority,
            })?)
            .clone();
            let child_sync_registries = sync_iter.cloned().collect();
            drop(sync_inner);

            let sync_container = child.sync.clone();
            child = child
                .init_child_with_context(
                    sync_container,
                    self.context.clone(),
                    registry,
                    child_registries,
                    sync_registry,
                    child_sync_registries,
                    true,
                )
                .await;
            inner = child.inner.lock().await;
            sync_inner = child.sync.inner.lock();
        }
        drop(inner);
        drop(sync_inner);

        Ok(child)
    }
}

struct BoxedContainerInner {
    cache: Cache,
    context: Context,
    root_registry: Arc<Registry>,
    child_registries: Box<[Arc<Registry>]>,
    parent: Option<Box<BoxedContainerInner>>,
    close_parent: bool,
    sync_container: LifetimedSyncContainerInner,
}

impl BoxedContainerInner {
    #[inline]
    #[must_use]
    fn init_child(
        self,
        root_registry: Arc<Registry>,
        child_registries: Box<[Arc<Registry>]>,
        close_parent: bool,
        sync_container: LifetimedSyncContainerInner,
    ) -> Self {
        let mut cache = self.cache.child();
        let context = self.context.clone();
        cache.append_context(&context);

        Self {
            cache,
            context,
            root_registry,
            child_registries,
            parent: Some(Box::new(self)),
            close_parent,
            sync_container,
        }
    }
}

impl From<BoxedContainerInner> for Container {
    fn from(
        BoxedContainerInner {
            cache,
            context,
            root_registry,
            child_registries,
            parent,
            close_parent,
            sync_container,
        }: BoxedContainerInner,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ContainerInner {
                cache,
                context,
                root_registry,
                child_registries,
                parent: parent.map(|parent| (*parent).into()),
                close_parent,
            })),
            sync: sync_container.into(),
        }
    }
}

#[derive(Clone)]
struct ContainerInner {
    cache: Cache,
    context: Context,
    root_registry: Arc<Registry>,
    child_registries: Box<[Arc<Registry>]>,
    parent: Option<Container>,
    close_parent: bool,
}

impl ContainerInner {
    /// Closes the container, calling finalizers for resolved dependencies in LIFO order.
    ///
    /// # Warning
    /// This method can be called multiple times, but it will only call finalizers for dependencies that were resolved since the last call
    #[allow(clippy::missing_panics_doc)]
    pub async fn close(&mut self) {
        self.close_with_parent_flag(self.close_parent).await;
    }

    #[allow(clippy::missing_panics_doc)]
    #[async_recursion]
    async fn close_with_parent_flag(&mut self, close_parent: bool) {
        while let Some(Resolved { type_id, dependency }) = self.cache.get_resolved_set_mut().0.pop_back() {
            let InstantiatorInnerData { finalizer, .. } = self
                .root_registry
                .get_instantiator_data(&type_id)
                .expect("Instantiator should be present for resolved type");

            if let Some(mut finalizer) = finalizer {
                let _ = finalizer.call(dependency).await;
                debug!(?type_id, "Finalizer called");
            }
        }

        // We need to clear cache and fill it with the context as in start of the container usage
        #[allow(clippy::assigning_clones)]
        {
            self.cache.map = self.context.map.clone();
        }

        if close_parent {
            if let Some(parent) = &self.parent {
                parent.close().await;
                debug!("Parent container closed");
            }
        }
    }
}

impl Drop for ContainerInner {
    fn drop(&mut self) {
        self.close();
        debug!("Container closed on drop");
    }
}

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    extern crate std;

    use super::{Container, ContainerInner, RegistriesBuilder};
    use crate::{
        async_impl::{Inject, InjectTransient},
        scope::DefaultScope::*,
        Inject as SyncInject, InjectTransient as SyncInjectTransient, Scope,
    };

    use alloc::{
        format,
        string::{String, ToString as _},
        sync::Arc,
    };
    use core::sync::atomic::{AtomicU8, Ordering};
    use tracing::debug;
    use tracing_test::traced_test;

    struct Request1;
    struct Request2(Arc<Request1>);
    struct Request3(Arc<Request1>, Arc<Request2>);

    #[tokio::test]
    #[traced_test]
    async fn test_scoped_get() {
        struct A(Arc<B>, Arc<C>);
        struct B(i32);
        struct C(Arc<CA>);
        struct CA(Arc<CAA>);
        struct CAA(Arc<CAAA>);
        struct CAAA(Arc<CAAAA>);
        struct CAAAA(Arc<CAAAAA>);
        struct CAAAAA;

        let registry = RegistriesBuilder::new()
            .provide(|| (Ok(CAAAAA)), Runtime)
            .provide(|SyncInject(caaaaa): SyncInject<CAAAAA>| Ok(CAAAA(caaaaa)), App)
            .provide(|SyncInject(caaaa): SyncInject<CAAAA>| Ok(CAAA(caaaa)), Session)
            .provide(|SyncInject(caaa): SyncInject<CAAA>| Ok(CAA(caaa)), Request)
            .provide(|SyncInject(caa): SyncInject<CAA>| Ok(CA(caa)), Request)
            .provide(|SyncInject(ca): SyncInject<CA>| Ok(C(ca)), Action)
            .provide(|| Ok(B(2)), App)
            .provide(|SyncInject(b): SyncInject<B>, SyncInject(c): SyncInject<C>| Ok(A(b, c)), Step);
        let app_container = Container::new(registry);
        let request_container = app_container.clone().enter_build().await.unwrap();
        let action_container = request_container.clone().enter_build().await.unwrap();
        let step_container = action_container.clone().enter_build().await.unwrap();

        let _ = step_container.get::<A>().await.unwrap();
        let _ = step_container.get::<CAAAAA>().await.unwrap();
        let _ = step_container.get::<CAAAA>().await.unwrap();
        let _ = step_container.get::<CAAA>().await.unwrap();
        let _ = step_container.get::<CAA>().await.unwrap();
        let _ = step_container.get::<CA>().await.unwrap();
        let _ = step_container.get::<C>().await.unwrap();
        let _ = step_container.get::<B>().await.unwrap();
    }

    #[tokio::test]
    #[traced_test]
    async fn test_async_scoped_get() {
        struct A(Arc<B>, Arc<C>);
        struct B(i32);
        struct C(Arc<CA>);
        struct CA(Arc<CAA>);
        struct CAA(Arc<CAAA>);
        struct CAAA(Arc<CAAAA>);
        struct CAAAA(Arc<CAAAAA>);
        struct CAAAAA;

        let registry = RegistriesBuilder::new()
            .provide_async(async || (Ok(CAAAAA)), Runtime)
            .provide_async(async |Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa)), App)
            .provide_async(async |Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa)), Session)
            .provide_async(async |Inject(caaa): Inject<CAAA>| Ok(CAA(caaa)), Request)
            .provide_async(async |Inject(caa): Inject<CAA>| Ok(CA(caa)), Request)
            .provide_async(async |Inject(ca): Inject<CA>| Ok(C(ca)), Action)
            .provide_async(async || Ok(B(2)), App)
            .provide_async(async |Inject(b): Inject<B>, Inject(c): Inject<C>| Ok(A(b, c)), Step);
        let app_container = Container::new(registry);
        let request_container = app_container.clone().enter_build().await.unwrap();
        let action_container = request_container.clone().enter_build().await.unwrap();
        let step_container = action_container.clone().enter_build().await.unwrap();

        let _ = step_container.get::<A>().await.unwrap();
        let _ = step_container.get::<CAAAAA>().await.unwrap();
        let _ = step_container.get::<CAAAA>().await.unwrap();
        let _ = step_container.get::<CAAA>().await.unwrap();
        let _ = step_container.get::<CAA>().await.unwrap();
        let _ = step_container.get::<CA>().await.unwrap();
        let _ = step_container.get::<C>().await.unwrap();
        let _ = step_container.get::<B>().await.unwrap();
    }

    struct RequestTransient1;
    struct RequestTransient2(RequestTransient1);
    struct RequestTransient3(RequestTransient1, RequestTransient2);

    #[tokio::test]
    #[traced_test]
    async fn test_transient_get() {
        let registry = RegistriesBuilder::new()
            .provide(|| Ok(RequestTransient1), App)
            .provide(
                |SyncInjectTransient(req): SyncInjectTransient<RequestTransient1>| Ok(RequestTransient2(req)),
                Request,
            )
            .provide(
                |SyncInjectTransient(req_1): SyncInjectTransient<RequestTransient1>,
                 SyncInjectTransient(req_2): SyncInjectTransient<RequestTransient2>| {
                    Ok(RequestTransient3(req_1, req_2))
                },
                Request,
            );
        let app_container = Container::new(registry);
        let request_container = app_container.clone().enter().with_scope(Request).build().await.unwrap();

        assert!(app_container.get_transient::<RequestTransient1>().await.is_ok());
        assert!(app_container.get_transient::<RequestTransient2>().await.is_err());
        assert!(app_container.get_transient::<RequestTransient3>().await.is_err());

        assert!(request_container.get_transient::<RequestTransient1>().await.is_ok());
        assert!(request_container.get_transient::<RequestTransient2>().await.is_ok());
        assert!(request_container.get_transient::<RequestTransient3>().await.is_ok());
    }

    #[tokio::test]
    #[traced_test]
    async fn test_async_transient_get() {
        let registry = RegistriesBuilder::new()
            .provide_async(async || Ok(RequestTransient1), App)
            .provide_async(
                async |InjectTransient(req): InjectTransient<RequestTransient1>| Ok(RequestTransient2(req)),
                Request,
            )
            .provide_async(
                async |InjectTransient(req_1): InjectTransient<RequestTransient1>,
                       InjectTransient(req_2): InjectTransient<RequestTransient2>| { Ok(RequestTransient3(req_1, req_2)) },
                Request,
            );
        let app_container = Container::new(registry);
        let request_container = app_container.clone().enter().with_scope(Request).build().await.unwrap();

        assert!(app_container.get_transient::<RequestTransient1>().await.is_ok());
        assert!(app_container.get_transient::<RequestTransient2>().await.is_err());
        assert!(app_container.get_transient::<RequestTransient3>().await.is_err());

        assert!(request_container.get_transient::<RequestTransient1>().await.is_ok());
        assert!(request_container.get_transient::<RequestTransient2>().await.is_ok());
        assert!(request_container.get_transient::<RequestTransient3>().await.is_ok());
    }

    #[tokio::test]
    #[traced_test]
    async fn test_scope_hierarchy() {
        let registry = RegistriesBuilder::new()
            .provide_async(async || Ok(()), Runtime)
            .provide_async(async || Ok(((), ())), App)
            .provide_async(async || Ok(((), (), ())), Session)
            .provide_async(async || Ok(((), (), (), ())), Request)
            .provide_async(async || Ok(((), (), (), (), ())), Action)
            .provide_async(async || Ok(((), (), (), (), (), ())), Step);

        let app_container = Container::new(registry);
        let request_container = app_container.clone().enter_build().await.unwrap();
        let action_container = request_container.clone().enter_build().await.unwrap();
        let step_container = action_container.clone().enter_build().await.unwrap();

        let app_container_inner = app_container.inner.lock().await;
        let request_container_inner = request_container.inner.lock().await;
        let action_container_inner = action_container.inner.lock().await;
        let step_container_inner = step_container.inner.lock().await;

        // Runtime scope is skipped by default, but it is still present in the parent
        assert_eq!(
            app_container_inner
                .parent
                .as_ref()
                .unwrap()
                .inner
                .lock()
                .await
                .root_registry
                .scope
                .priority,
            Runtime.priority()
        );
        assert_eq!(app_container_inner.child_registries.len(), 4);
        assert_eq!(app_container_inner.root_registry.scope.priority, App.priority());

        // Session scope is skipped by default, but it is still present in the child registries
        assert_eq!(
            request_container_inner
                .parent
                .as_ref()
                .unwrap()
                .inner
                .lock()
                .await
                .root_registry
                .scope
                .priority,
            Session.priority()
        );
        assert_eq!(request_container_inner.child_registries.len(), 2);
        assert_eq!(request_container_inner.root_registry.scope.priority, Request.priority());
        // Session scope is skipped by default, so it is not the first child registry
        assert!(Arc::ptr_eq(
            &request_container_inner.root_registry,
            &app_container_inner.child_registries[1]
        ));
        assert!(Arc::ptr_eq(
            &action_container_inner.root_registry,
            &request_container_inner.child_registries[0]
        ));

        assert_eq!(action_container_inner.child_registries.len(), 1);
        assert_eq!(action_container_inner.root_registry.scope.priority, Action.priority());

        assert_eq!(step_container_inner.child_registries.len(), 0);
        assert_eq!(step_container_inner.root_registry.scope.priority, Step.priority());
        assert!(Arc::ptr_eq(
            &step_container_inner.root_registry,
            &action_container_inner.child_registries[0]
        ));
    }

    #[tokio::test]
    #[traced_test]
    async fn test_scope_with_hierarchy() {
        let registry = RegistriesBuilder::new()
            .provide_async(async || Ok(()), Runtime)
            .provide_async(async || Ok(((), ())), App)
            .provide_async(async || Ok(((), (), ())), Session)
            .provide_async(async || Ok(((), (), (), ())), Request)
            .provide_async(async || Ok(((), (), (), (), ())), Action)
            .provide_async(async || Ok(((), (), (), (), (), ())), Step);

        let runtime_container = Container::new_with_start_scope(registry, Runtime);
        let app_container = runtime_container.clone().enter().with_scope(App).build().await.unwrap();
        let session_container = runtime_container.clone().enter().with_scope(Session).build().await.unwrap();
        let request_container = app_container.clone().enter().with_scope(Request).build().await.unwrap();
        let action_container = request_container.clone().enter().with_scope(Action).build().await.unwrap();
        let step_container = action_container.clone().enter().with_scope(Step).build().await.unwrap();

        let runtime_container_inner = runtime_container.inner.lock().await;
        let app_container_inner = app_container.inner.lock().await;
        let session_container_inner = session_container.inner.lock().await;
        let request_container_inner = request_container.inner.lock().await;
        let action_container_inner = action_container.inner.lock().await;
        let step_container_inner = step_container.inner.lock().await;

        assert!(runtime_container_inner.parent.is_none());
        assert_eq!(runtime_container_inner.child_registries.len(), 5);
        assert_eq!(runtime_container_inner.root_registry.scope.priority, Runtime.priority());
        assert!(Arc::ptr_eq(
            &app_container_inner.root_registry,
            &runtime_container_inner.child_registries[0]
        ));

        assert_eq!(app_container_inner.child_registries.len(), 4);
        assert_eq!(app_container_inner.root_registry.scope.priority, App.priority());
        assert!(Arc::ptr_eq(
            &session_container_inner.root_registry,
            &app_container_inner.child_registries[0]
        ));

        assert_eq!(session_container_inner.child_registries.len(), 3);
        assert_eq!(session_container_inner.root_registry.scope.priority, Session.priority());
        assert!(Arc::ptr_eq(
            &request_container_inner.root_registry,
            &session_container_inner.child_registries[0]
        ));

        assert_eq!(request_container_inner.child_registries.len(), 2);
        assert_eq!(request_container_inner.root_registry.scope.priority, Request.priority());
        assert!(Arc::ptr_eq(
            &action_container_inner.root_registry,
            &request_container_inner.child_registries[0]
        ));

        assert_eq!(action_container_inner.child_registries.len(), 1);
        assert_eq!(action_container_inner.root_registry.scope.priority, Action.priority());
        assert!(Arc::ptr_eq(
            &step_container_inner.root_registry,
            &action_container_inner.child_registries[0]
        ));

        assert_eq!(step_container_inner.child_registries.len(), 0);
        assert_eq!(step_container_inner.root_registry.scope.priority, Step.priority());
    }

    #[tokio::test]
    #[traced_test]
    async fn test_close_for_unresolved() {
        let finalizer_1_request_call_count = Arc::new(AtomicU8::new(0));
        let finalizer_2_request_call_count = Arc::new(AtomicU8::new(0));
        let finalizer_3_request_call_count = Arc::new(AtomicU8::new(0));
        let finalizer_4_request_call_count = Arc::new(AtomicU8::new(0));

        let registry = RegistriesBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(((), ())), App)
            .provide_async(async || Ok(((), (), ())), Session)
            .provide(|| Ok(((), (), (), ())), Request)
            .add_finalizer({
                let finalizer_1_request_call_count = finalizer_1_request_call_count.clone();
                move |_: Arc<()>| {
                    finalizer_1_request_call_count.fetch_add(1, Ordering::SeqCst);
                }
            })
            .add_finalizer({
                let finalizer_2_request_call_count = finalizer_2_request_call_count.clone();
                move |_: Arc<((), ())>| {
                    finalizer_2_request_call_count.fetch_add(1, Ordering::SeqCst);
                }
            })
            .add_async_finalizer({
                let finalizer_3_request_call_count = finalizer_3_request_call_count.clone();
                move |_: Arc<((), (), ())>| {
                    let finalizer_3_request_call_count = finalizer_3_request_call_count.clone();
                    async move {
                        finalizer_3_request_call_count.fetch_add(1, Ordering::SeqCst);
                    }
                }
            })
            .add_finalizer({
                let finalizer_4_request_call_count = finalizer_4_request_call_count.clone();
                move |_: Arc<((), (), (), ())>| {
                    finalizer_4_request_call_count.fetch_add(1, Ordering::SeqCst);
                }
            });

        let app_container = Container::new(registry);
        let session_container = app_container.clone().enter().with_scope(Session).build().await.unwrap();
        let request_container = app_container.clone().enter().with_scope(Request).build().await.unwrap();

        session_container.close().await;
        request_container.close().await;
        app_container.close().await;

        assert_eq!(finalizer_1_request_call_count.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_2_request_call_count.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_3_request_call_count.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_4_request_call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_close_for_resolved() {
        let request_call_count = Arc::new(AtomicU8::new(0));

        let finalizer_1_request_call_count = Arc::new(AtomicU8::new(0));
        let finalizer_1_request_call_position = Arc::new(AtomicU8::new(0));
        let finalizer_2_request_call_count = Arc::new(AtomicU8::new(0));
        let finalizer_2_request_call_position = Arc::new(AtomicU8::new(0));
        let finalizer_3_request_call_count = Arc::new(AtomicU8::new(0));
        let finalizer_3_request_call_position = Arc::new(AtomicU8::new(0));
        let finalizer_4_request_call_count = Arc::new(AtomicU8::new(0));
        let finalizer_4_request_call_position = Arc::new(AtomicU8::new(0));
        let finalizer_5_request_call_count = Arc::new(AtomicU8::new(0));
        let finalizer_5_request_call_position = Arc::new(AtomicU8::new(0));

        let registry = RegistriesBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(((), ())), App)
            .provide_async(async || Ok(((), (), ())), App)
            .provide(|| Ok(((), (), (), ())), Request)
            .provide(|| Ok(((), (), (), (), ())), Request)
            .add_finalizer({
                let request_call_count = request_call_count.clone();
                let finalizer_1_request_call_position = finalizer_1_request_call_position.clone();
                let finalizer_1_request_call_count = finalizer_1_request_call_count.clone();
                move |_: Arc<()>| {
                    request_call_count.fetch_add(1, Ordering::SeqCst);
                    finalizer_1_request_call_position.store(request_call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                    finalizer_1_request_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Finalizer 1 called");
                }
            })
            .add_finalizer({
                let request_call_count = request_call_count.clone();
                let finalizer_2_request_call_position = finalizer_2_request_call_position.clone();
                let finalizer_2_request_call_count = finalizer_2_request_call_count.clone();
                move |_: Arc<((), ())>| {
                    request_call_count.fetch_add(1, Ordering::SeqCst);
                    finalizer_2_request_call_position.store(request_call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                    finalizer_2_request_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Finalizer 2 called");
                }
            })
            .add_async_finalizer({
                let request_call_count = request_call_count.clone();
                let finalizer_3_request_call_position = finalizer_3_request_call_position.clone();
                let finalizer_3_request_call_count = finalizer_3_request_call_count.clone();
                move |_: Arc<((), (), ())>| {
                    let request_call_count = request_call_count.clone();
                    let finalizer_3_request_call_position = finalizer_3_request_call_position.clone();
                    let finalizer_3_request_call_count = finalizer_3_request_call_count.clone();
                    async move {
                        request_call_count.fetch_add(1, Ordering::SeqCst);
                        finalizer_3_request_call_position.store(request_call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                        finalizer_3_request_call_count.fetch_add(1, Ordering::SeqCst);

                        debug!("Finalizer 3 called");
                    }
                }
            })
            .add_finalizer({
                let request_call_count = request_call_count.clone();
                let finalizer_4_request_call_position = finalizer_4_request_call_position.clone();
                let finalizer_4_request_call_count = finalizer_4_request_call_count.clone();
                move |_: Arc<((), (), (), ())>| {
                    request_call_count.fetch_add(1, Ordering::SeqCst);
                    finalizer_4_request_call_position.store(request_call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                    finalizer_4_request_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Finalizer 4 called");
                }
            })
            .add_finalizer({
                let request_call_count = request_call_count.clone();
                let finalizer_5_request_call_position = finalizer_5_request_call_position.clone();
                let finalizer_5_request_call_count = finalizer_5_request_call_count.clone();
                move |_: Arc<((), (), (), (), ())>| {
                    request_call_count.fetch_add(1, Ordering::SeqCst);
                    finalizer_5_request_call_position.store(request_call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                    finalizer_5_request_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Finalizer 5 called");
                }
            });

        let app_container = Container::new(registry);
        let request_container = app_container.clone().enter().with_scope(Request).build().await.unwrap();

        let _ = request_container.get::<()>().await.unwrap();
        let _ = request_container.get::<((), ())>().await.unwrap();
        let _ = request_container.get::<((), (), ())>().await.unwrap();
        let _ = request_container.get::<((), (), (), (), ())>().await.unwrap();
        let _ = request_container.get::<((), (), (), ())>().await.unwrap();

        let runtime_container_resolved_set_count = app_container
            .inner
            .lock()
            .await
            .parent
            .as_ref()
            .unwrap()
            .sync
            .inner
            .lock()
            .cache
            .get_resolved_set()
            .0
            .len();
        let app_container_resolved_set_count = app_container.sync.inner.lock().cache.get_resolved_set().0.len()
            + app_container.inner.lock().await.cache.get_resolved_set().0.len();
        let request_container_resolved_set_count = request_container.sync.inner.lock().cache.get_resolved_set().0.len();

        request_container.close().await;

        assert_eq!(runtime_container_resolved_set_count, 1);
        assert_eq!(app_container_resolved_set_count, 2);
        assert_eq!(request_container_resolved_set_count, 2);

        assert_eq!(finalizer_1_request_call_count.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_1_request_call_position.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_2_request_call_count.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_2_request_call_position.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_3_request_call_count.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_3_request_call_position.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_4_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_4_request_call_position.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_5_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_5_request_call_position.load(Ordering::SeqCst), 2);

        app_container.close().await;

        assert_eq!(finalizer_1_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_1_request_call_position.load(Ordering::SeqCst), 5);
        assert_eq!(finalizer_2_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_2_request_call_position.load(Ordering::SeqCst), 4);
        assert_eq!(finalizer_3_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_3_request_call_position.load(Ordering::SeqCst), 3);
        assert_eq!(finalizer_4_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_4_request_call_position.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_5_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_5_request_call_position.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_close_on_drop() {
        let call_count = Arc::new(AtomicU8::new(0));

        let drop_call_count = Arc::new(AtomicU8::new(0));
        let drop_call_position = Arc::new(AtomicU8::new(0));
        let instantiator_call_count = Arc::new(AtomicU8::new(0));
        let instantiator_call_position = Arc::new(AtomicU8::new(0));
        let finalizer_1_call_count = Arc::new(AtomicU8::new(0));
        let finalizer_1_call_position = Arc::new(AtomicU8::new(0));
        let finalizer_2_call_count = Arc::new(AtomicU8::new(0));
        let finalizer_2_call_position = Arc::new(AtomicU8::new(0));

        struct Type1;
        struct Type2(Arc<Type1>);

        struct DropWrapper<T> {
            val: T,
            call_count: Arc<AtomicU8>,
            drop_call_count: Arc<AtomicU8>,
            drop_call_position: Arc<AtomicU8>,
        }

        impl<T> Drop for DropWrapper<T> {
            fn drop(&mut self) {
                self.call_count.fetch_add(1, Ordering::SeqCst);
                self.drop_call_count.fetch_add(1, Ordering::SeqCst);
                self.drop_call_position
                    .store(self.call_count.load(Ordering::SeqCst), Ordering::SeqCst);

                debug!("Drop called");
            }
        }

        let registry = RegistriesBuilder::new()
            .provide(|| Ok(Type1), App)
            .provide(|SyncInject(type_1): SyncInject<Type1>| Ok(Type2(type_1)), Request)
            .add_finalizer({
                let call_count = call_count.clone();
                let finalizer_1_call_count = finalizer_1_call_count.clone();
                let finalizer_1_call_position = finalizer_1_call_position.clone();
                move |_: Arc<Type1>| {
                    call_count.fetch_add(1, Ordering::SeqCst);
                    finalizer_1_call_position.store(call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                    finalizer_1_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Finalizer 1 called");
                }
            })
            .add_finalizer({
                let call_count = call_count.clone();
                let finalizer_2_call_count = finalizer_2_call_count.clone();
                let finalizer_2_call_position = finalizer_2_call_position.clone();
                move |_: Arc<Type2>| {
                    call_count.fetch_add(1, Ordering::SeqCst);
                    finalizer_2_call_position.store(call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                    finalizer_2_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Finalizer 2 called");
                }
            });

        let app_container = Container::new(registry);
        let request_container = app_container.enter_build().await.unwrap();
        DropWrapper {
            val: request_container,
            call_count: call_count.clone(),
            drop_call_count: drop_call_count.clone(),
            drop_call_position: drop_call_position.clone(),
        }
        .val
        .get::<Type2>()
        .await
        .unwrap();

        instantiator_call_position.store(call_count.load(Ordering::SeqCst) + 1, Ordering::SeqCst);
        instantiator_call_count.fetch_add(1, Ordering::SeqCst);

        debug!("Instantiator called");

        assert_eq!(call_count.load(Ordering::SeqCst), 3);
        assert_eq!(finalizer_1_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_1_call_position.load(Ordering::SeqCst), 3);
        assert_eq!(finalizer_2_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_2_call_position.load(Ordering::SeqCst), 2);
        assert_eq!(instantiator_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(instantiator_call_position.load(Ordering::SeqCst), 4);
        assert_eq!(drop_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(drop_call_position.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_bounds() {
        fn impl_bounds<T: Send + Sync + 'static>() {}

        impl_bounds::<(Container, ContainerInner)>();
    }
}
