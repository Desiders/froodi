use alloc::boxed::Box;
use async_recursion::async_recursion;
use core::any::{type_name, TypeId};
use parking_lot::Mutex;
use tracing::{debug, debug_span, error};

use super::{
    registry::{InstantiatorInnerData, RegistryBuilder, ScopedRegistry},
    service::Service as _,
};
use crate::{
    cache::{Cache, Resolved},
    container::{BoxedContainerInner as BoxedSyncContainerInner, Container as SyncContainer, ContainerInner as SyncContainerInner},
    context::Context,
    errors::{InstantiatorErrorKind, ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind},
    registry::ScopedRegistry as SyncRegistry,
    scope::Scope,
    utils::thread_safety::{RcThreadSafety, SendSafety, SyncSafety},
};

#[derive(Clone)]
pub struct Container {
    inner: RcThreadSafety<ContainerInner>,
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
    pub fn new<S: Scope + Clone>(registry_builder: RegistryBuilder<S>) -> Self {
        let (registries, sync_registries) = registry_builder.build();
        let mut registries = registries.into_iter();
        let (root_registry, child_registries) = if let Some(root_registry) = registries.next() {
            (RcThreadSafety::new(root_registry), registries.map(RcThreadSafety::new).collect())
        } else {
            panic!("registries len (is 0) should be > 1");
        };
        let mut sync_registries = sync_registries.into_iter();
        let (root_sync_registry, child_sync_registries) = if let Some(root_registry) = sync_registries.next() {
            (
                RcThreadSafety::new(root_registry),
                sync_registries.map(RcThreadSafety::new).collect(),
            )
        } else {
            panic!("registries len (is 0) should be > 1");
        };

        let mut sync_container = BoxedSyncContainerInner {
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
    pub fn new_with_start_scope<S: Scope + Clone>(registry_builder: RegistryBuilder<S>, scope: S) -> Self {
        let (registries, sync_registries) = registry_builder.build();
        let mut registries = registries.into_iter();
        let (root_registry, child_registries) = if let Some(root_registry) = registries.next() {
            (RcThreadSafety::new(root_registry), registries.map(RcThreadSafety::new).collect())
        } else {
            panic!("registries len (is 0) should be > 1");
        };
        let mut sync_registries = sync_registries.into_iter();
        let (root_sync_registry, child_sync_registries) = if let Some(root_registry) = sync_registries.next() {
            (
                RcThreadSafety::new(root_registry),
                sync_registries.map(RcThreadSafety::new).collect(),
            )
        } else {
            panic!("registries len (is 0) should be > 1");
        };

        let mut container_priority = root_registry.scope.priority;
        let priority = scope.priority();

        let mut sync_container = BoxedSyncContainerInner {
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
    pub fn enter_build(self) -> Result<Container, ScopeErrorKind> {
        self.enter().build()
    }

    /// Gets a scoped dependency from the container
    ///
    /// # Notes
    /// This method resolves a dependency from the container,
    /// so it should be used for dependencies that are cached or shared,
    /// and with optional finalizer.
    #[allow(clippy::missing_errors_doc, clippy::multiple_bound_locations)]
    #[async_recursion]
    pub async fn get<Dep: SendSafety + SyncSafety + 'static>(&self) -> Result<RcThreadSafety<Dep>, ResolveErrorKind> {
        let span = debug_span!("resolve", dependency = type_name::<Dep>());
        let _guard = span.enter();

        let type_id = TypeId::of::<Dep>();

        if let Some(dependency) = self.inner.cache.lock().get(&type_id) {
            debug!("Found in cache");
            return Ok(dependency);
        }
        debug!("Not found in cache");

        let Some(InstantiatorInnerData {
            mut instantiator,
            finalizer,
            config,
        }) = self.inner.root_registry.get_instantiator_data(&type_id)
        else {
            if let Some(parent) = &self.inner.parent {
                debug!("No instantiator found, trying parent container");
                return match parent.get::<Dep>().await {
                    Ok(dependency) => {
                        self.inner.cache.lock().insert_rc(dependency.clone());
                        Ok(dependency)
                    }
                    Err(_err) => {
                        debug!("No instantiator found, trying sync container");
                        return self.sync.get();
                    }
                };
            }

            debug!("No instantiator found, trying sync container");
            return self.sync.get();
        };

        match instantiator.call(self.clone()).await {
            Ok(dependency) => match dependency.downcast::<Dep>() {
                Ok(dependency) => {
                    let dependency = RcThreadSafety::new(*dependency);
                    let mut guard = self.inner.cache.lock();
                    if config.cache_provides {
                        guard.insert_rc(dependency.clone());
                        debug!("Cached");
                    }
                    if finalizer.is_some() {
                        guard.push_resolved(Resolved {
                            type_id,
                            dependency: dependency.clone(),
                        });
                        debug!("Pushed to resolved set");
                    }
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

        let Some(mut instantiator) = self.inner.root_registry.get_instantiator(&type_id) else {
            if let Some(parent) = &self.inner.parent {
                debug!("No instantiator found, trying parent container");
                return match parent.get_transient().await {
                    Ok(dependency) => Ok(dependency),
                    Err(_err) => {
                        debug!("No instantiator found, trying sync container");
                        self.sync.get_transient()
                    }
                };
            }

            debug!("No instantiator found, trying sync container");
            return self.sync.get_transient();
        };

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
        let close_parent = self.inner.close_parent;

        self.inner.close_with_parent_flag(false).await;
        self.sync.inner.close_with_parent_flag(false);

        if close_parent {
            if let Some(parent) = &self.inner.parent {
                parent.close().await;
            }
        }
    }
}

impl Container {
    #[inline]
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    fn init_child_with_context(
        self,
        sync_container: SyncContainer,
        context: Context,
        root_registry: RcThreadSafety<ScopedRegistry>,
        child_registries: Box<[RcThreadSafety<ScopedRegistry>]>,
        root_sync_registry: RcThreadSafety<SyncRegistry>,
        child_sync_registries: Box<[RcThreadSafety<SyncRegistry>]>,
        close_parent: bool,
    ) -> Self {
        let mut cache = self.inner.cache.lock().child();
        cache.append_context(&mut context.clone());

        let mut sync_cache = self.sync.inner.cache.lock().child();
        sync_cache.append_context(&mut context.clone());

        Self {
            inner: RcThreadSafety::new(ContainerInner {
                cache: Mutex::new(cache),
                context: Mutex::new(context.clone()),
                root_registry,
                child_registries,
                parent: Some(self),
                close_parent,
            }),
            sync: SyncContainer {
                inner: RcThreadSafety::new(SyncContainerInner {
                    cache: Mutex::new(sync_cache),
                    context: Mutex::new(context),
                    root_registry: root_sync_registry,
                    child_registries: child_sync_registries,
                    parent: Some(sync_container),
                    close_parent,
                }),
            },
        }
    }

    #[inline]
    #[must_use]
    fn init_child(
        self,
        sync_container: SyncContainer,
        root_registry: RcThreadSafety<ScopedRegistry>,
        child_registries: Box<[RcThreadSafety<ScopedRegistry>]>,
        root_sync_registry: RcThreadSafety<SyncRegistry>,
        child_sync_registries: Box<[RcThreadSafety<SyncRegistry>]>,
        close_parent: bool,
    ) -> Self {
        let mut cache = self.inner.cache.lock().child();
        let context = self.inner.context.lock().clone();
        cache.append_context(&mut context.clone());

        let mut sync_cache = self.sync.inner.cache.lock().child();
        let sync_context = self.sync.inner.context.lock().clone();
        sync_cache.append_context(&mut sync_context.clone());

        Self {
            inner: RcThreadSafety::new(ContainerInner {
                cache: Mutex::new(cache),
                context: Mutex::new(context),
                root_registry,
                child_registries,
                parent: Some(self),
                close_parent,
            }),
            sync: SyncContainer {
                inner: RcThreadSafety::new(SyncContainerInner {
                    cache: Mutex::new(sync_cache),
                    context: Mutex::new(sync_context),
                    root_registry: root_sync_registry,
                    child_registries: child_sync_registries,
                    parent: Some(sync_container),
                    close_parent,
                }),
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
    pub fn build(self) -> Result<Container, ScopeErrorKind> {
        use ScopeErrorKind::{NoChildRegistries, NoNonSkippedRegistries};

        let mut iter = self.container.inner.child_registries.iter();
        let registry = (*iter.next().ok_or(NoChildRegistries)?).clone();
        let child_registries = iter.cloned().collect();

        let mut sync_iter = self.container.sync.inner.child_registries.iter();
        let sync_registry = (*sync_iter.next().ok_or(NoChildRegistries)?).clone();
        let child_sync_registries = sync_iter.cloned().collect();

        let sync_container = self.container.sync.clone();
        let mut child = self.container.init_child(
            sync_container,
            registry,
            child_registries,
            sync_registry,
            child_sync_registries,
            false,
        );
        while child.inner.root_registry.scope.is_skipped_by_default {
            let mut iter = child.inner.child_registries.iter();
            let registry = (*iter.next().ok_or(NoNonSkippedRegistries)?).clone();
            let child_registries = iter.cloned().collect();

            let mut sync_iter = child.sync.inner.child_registries.iter();
            let sync_registry = (*sync_iter.next().ok_or(NoNonSkippedRegistries)?).clone();
            let child_sync_registries = sync_iter.cloned().collect();

            let sync_container = child.sync.clone();
            child = child.init_child(
                sync_container,
                registry,
                child_registries,
                sync_registry,
                child_sync_registries,
                true,
            );
        }

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
    pub fn build(self) -> Result<Container, ScopeWithErrorKind> {
        use ScopeWithErrorKind::{NoChildRegistries, NoChildRegistriesWithScope};

        let priority = self.scope.priority();

        let mut iter = self.container.inner.child_registries.iter();
        let registry = (*iter.next().ok_or(NoChildRegistries)?).clone();
        let child_registries = iter.cloned().collect();

        let mut sync_iter = self.container.sync.inner.child_registries.iter();
        let sync_registry = (*sync_iter.next().ok_or(NoChildRegistries)?).clone();
        let child_sync_registries = sync_iter.cloned().collect();

        let sync_container = self.container.sync.clone();
        let mut child = self.container.init_child(
            sync_container,
            registry,
            child_registries,
            sync_registry,
            child_sync_registries,
            false,
        );
        while child.inner.root_registry.scope.priority != priority {
            let mut iter = child.inner.child_registries.iter();
            let registry = (*iter.next().ok_or(NoChildRegistriesWithScope {
                name: self.scope.name(),
                priority,
            })?)
            .clone();
            let child_registries = iter.cloned().collect();

            let mut sync_iter = child.sync.inner.child_registries.iter();
            let sync_registry = (*sync_iter.next().ok_or(NoChildRegistriesWithScope {
                name: self.scope.name(),
                priority,
            })?)
            .clone();
            let child_sync_registries = sync_iter.cloned().collect();

            let sync_container = child.sync.clone();
            child = child.init_child(
                sync_container,
                registry,
                child_registries,
                sync_registry,
                child_sync_registries,
                true,
            );
        }

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
    pub fn build(self) -> Result<Container, ScopeErrorKind> {
        use ScopeErrorKind::{NoChildRegistries, NoNonSkippedRegistries};

        let mut iter = self.container.inner.child_registries.iter();
        let registry = (*iter.next().ok_or(NoChildRegistries)?).clone();
        let child_registries = iter.cloned().collect();

        let mut sync_iter = self.container.sync.inner.child_registries.iter();
        let sync_registry = (*sync_iter.next().ok_or(NoChildRegistries)?).clone();
        let child_sync_registries = sync_iter.cloned().collect();

        let sync_container = self.container.sync.clone();
        let mut child = self.container.init_child_with_context(
            sync_container,
            self.context.clone(),
            registry,
            child_registries,
            sync_registry,
            child_sync_registries,
            false,
        );
        while child.inner.root_registry.scope.is_skipped_by_default {
            let mut iter = child.inner.child_registries.iter();
            let registry = (*iter.next().ok_or(NoNonSkippedRegistries)?).clone();
            let child_registries = iter.cloned().collect();

            let mut sync_iter = child.sync.inner.child_registries.iter();
            let sync_registry = (*sync_iter.next().ok_or(NoNonSkippedRegistries)?).clone();
            let child_sync_registries = sync_iter.cloned().collect();

            let sync_container = child.sync.clone();
            child = child.init_child_with_context(
                sync_container,
                self.context.clone(),
                registry,
                child_registries,
                sync_registry,
                child_sync_registries,
                true,
            );
        }

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
    pub fn build(self) -> Result<Container, ScopeWithErrorKind> {
        use ScopeWithErrorKind::{NoChildRegistries, NoChildRegistriesWithScope};

        let priority = self.scope.priority();

        let mut iter = self.container.inner.child_registries.iter();
        let registry = (*iter.next().ok_or(NoChildRegistries)?).clone();
        let child_registries = iter.cloned().collect();

        let mut sync_iter = self.container.sync.inner.child_registries.iter();
        let sync_registry = (*sync_iter.next().ok_or(NoChildRegistries)?).clone();
        let child_sync_registries = sync_iter.cloned().collect();

        let sync_container = self.container.sync.clone();
        let mut child = self.container.init_child_with_context(
            sync_container,
            self.context.clone(),
            registry,
            child_registries,
            sync_registry,
            child_sync_registries,
            false,
        );
        while child.inner.root_registry.scope.priority != priority {
            let mut iter = child.inner.child_registries.iter();
            let registry = (*iter.next().ok_or(NoChildRegistriesWithScope {
                name: self.scope.name(),
                priority,
            })?)
            .clone();
            let child_registries = iter.cloned().collect();

            let mut sync_iter = child.sync.inner.child_registries.iter();
            let sync_registry = (*sync_iter.next().ok_or(NoChildRegistriesWithScope {
                name: self.scope.name(),
                priority,
            })?)
            .clone();
            let child_sync_registries = sync_iter.cloned().collect();

            let sync_container = child.sync.clone();
            child = child.init_child_with_context(
                sync_container,
                self.context.clone(),
                registry,
                child_registries,
                sync_registry,
                child_sync_registries,
                true,
            );
        }

        Ok(child)
    }
}

struct BoxedContainerInner {
    cache: Cache,
    context: Context,
    root_registry: RcThreadSafety<ScopedRegistry>,
    child_registries: Box<[RcThreadSafety<ScopedRegistry>]>,
    parent: Option<Box<BoxedContainerInner>>,
    close_parent: bool,
    sync_container: BoxedSyncContainerInner,
}

impl BoxedContainerInner {
    #[inline]
    #[must_use]
    fn init_child(
        self,
        root_registry: RcThreadSafety<ScopedRegistry>,
        child_registries: Box<[RcThreadSafety<ScopedRegistry>]>,
        close_parent: bool,
        sync_container: BoxedSyncContainerInner,
    ) -> Self {
        let mut cache = self.cache.child();
        let context = self.context.clone();
        cache.append_context(&mut context.clone());

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
            inner: RcThreadSafety::new(ContainerInner {
                cache: Mutex::new(cache),
                context: Mutex::new(context),
                root_registry,
                child_registries,
                parent: parent.map(|parent| (*parent).into()),
                close_parent,
            }),
            sync: sync_container.into(),
        }
    }
}

struct ContainerInner {
    cache: Mutex<Cache>,
    context: Mutex<Context>,
    root_registry: RcThreadSafety<ScopedRegistry>,
    child_registries: Box<[RcThreadSafety<ScopedRegistry>]>,
    parent: Option<Container>,
    close_parent: bool,
}

impl ContainerInner {
    #[allow(clippy::missing_panics_doc)]
    #[async_recursion]
    async fn close_with_parent_flag(&self, close_parent: bool) {
        let mut resolved_set = self.cache.lock().take_resolved_set();
        while let Some(Resolved { type_id, dependency }) = resolved_set.0.pop_back() {
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
            self.cache.lock().map = self.context.lock().map.clone();
        }

        if close_parent {
            if let Some(parent) = &self.parent {
                parent.close().await;
                debug!("Parent container closed");
            }
        }
    }
}

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    extern crate std;

    use super::{Container, ContainerInner, RegistryBuilder};
    use crate::{scope::DefaultScope::*, utils::thread_safety::RcThreadSafety, Inject, InjectTransient, Scope};

    use alloc::{
        format,
        string::{String, ToString as _},
    };
    use core::sync::atomic::{AtomicU8, Ordering};
    use tracing::debug;
    use tracing_test::traced_test;

    struct Request1;
    struct Request2(RcThreadSafety<Request1>);
    struct Request3(RcThreadSafety<Request1>, RcThreadSafety<Request2>);

    #[tokio::test]
    #[traced_test]
    async fn test_scoped_get() {
        struct A(RcThreadSafety<B>, RcThreadSafety<C>);
        struct B(i32);
        struct C(RcThreadSafety<CA>);
        struct CA(RcThreadSafety<CAA>);
        struct CAA(RcThreadSafety<CAAA>);
        struct CAAA(RcThreadSafety<CAAAA>);
        struct CAAAA(RcThreadSafety<CAAAAA>);
        struct CAAAAA;

        let registry = RegistryBuilder::new()
            .provide(|| (Ok(CAAAAA)), Runtime)
            .provide(|Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa)), App)
            .provide(|Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa)), Session)
            .provide(|Inject(caaa): Inject<CAAA>| Ok(CAA(caaa)), Request)
            .provide(|Inject(caa): Inject<CAA>| Ok(CA(caa)), Request)
            .provide(|Inject(ca): Inject<CA>| Ok(C(ca)), Action)
            .provide(|| Ok(B(2)), App)
            .provide(|Inject(b): Inject<B>, Inject(c): Inject<C>| Ok(A(b, c)), Step);
        let app_container = Container::new(registry);
        let request_container = app_container.clone().enter_build().unwrap();
        let action_container = request_container.clone().enter_build().unwrap();
        let step_container = action_container.clone().enter_build().unwrap();

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
        struct A(RcThreadSafety<B>, RcThreadSafety<C>);
        struct B(i32);
        struct C(RcThreadSafety<CA>);
        struct CA(RcThreadSafety<CAA>);
        struct CAA(RcThreadSafety<CAAA>);
        struct CAAA(RcThreadSafety<CAAAA>);
        struct CAAAA(RcThreadSafety<CAAAAA>);
        struct CAAAAA;

        let registry = RegistryBuilder::new()
            .provide_async(async || (Ok(CAAAAA)), Runtime)
            .provide_async(async |Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa)), App)
            .provide_async(async |Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa)), Session)
            .provide_async(async |Inject(caaa): Inject<CAAA>| Ok(CAA(caaa)), Request)
            .provide_async(async |Inject(caa): Inject<CAA>| Ok(CA(caa)), Request)
            .provide_async(async |Inject(ca): Inject<CA>| Ok(C(ca)), Action)
            .provide_async(async || Ok(B(2)), App)
            .provide_async(async |Inject(b): Inject<B>, Inject(c): Inject<C>| Ok(A(b, c)), Step);
        let app_container = Container::new(registry);
        let request_container = app_container.clone().enter_build().unwrap();
        let action_container = request_container.clone().enter_build().unwrap();
        let step_container = action_container.clone().enter_build().unwrap();

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
        let registry = RegistryBuilder::new()
            .provide(|| Ok(RequestTransient1), App)
            .provide(
                |InjectTransient(req): InjectTransient<RequestTransient1>| Ok(RequestTransient2(req)),
                Request,
            )
            .provide(
                |InjectTransient(req_1): InjectTransient<RequestTransient1>, InjectTransient(req_2): InjectTransient<RequestTransient2>| {
                    Ok(RequestTransient3(req_1, req_2))
                },
                Request,
            );
        let app_container = Container::new(registry);
        let request_container = app_container.clone().enter().with_scope(Request).build().unwrap();

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
        let registry = RegistryBuilder::new()
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
        let request_container = app_container.clone().enter().with_scope(Request).build().unwrap();

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
        let registry = RegistryBuilder::new()
            .provide_async(async || Ok(()), Runtime)
            .provide_async(async || Ok(((), ())), App)
            .provide_async(async || Ok(((), (), ())), Session)
            .provide_async(async || Ok(((), (), (), ())), Request)
            .provide_async(async || Ok(((), (), (), (), ())), Action)
            .provide_async(async || Ok(((), (), (), (), (), ())), Step);

        let app_container = Container::new(registry);
        let request_container = app_container.clone().enter_build().unwrap();
        let action_container = request_container.clone().enter_build().unwrap();
        let step_container = action_container.clone().enter_build().unwrap();

        let app_container_inner = app_container.inner;
        let request_container_inner = request_container.inner;
        let action_container_inner = action_container.inner;
        let step_container_inner = step_container.inner;

        // Runtime scope is skipped by default, but it is still present in the parent
        assert_eq!(
            app_container_inner.parent.as_ref().unwrap().inner.root_registry.scope.priority,
            Runtime.priority()
        );
        assert_eq!(app_container_inner.child_registries.len(), 4);
        assert_eq!(app_container_inner.root_registry.scope.priority, App.priority());

        // Session scope is skipped by default, but it is still present in the child registries
        assert_eq!(
            request_container_inner.parent.as_ref().unwrap().inner.root_registry.scope.priority,
            Session.priority()
        );
        assert_eq!(request_container_inner.child_registries.len(), 2);
        assert_eq!(request_container_inner.root_registry.scope.priority, Request.priority());
        // Session scope is skipped by default, so it is not the first child registry
        assert!(RcThreadSafety::ptr_eq(
            &request_container_inner.root_registry,
            &app_container_inner.child_registries[1]
        ));
        assert!(RcThreadSafety::ptr_eq(
            &action_container_inner.root_registry,
            &request_container_inner.child_registries[0]
        ));

        assert_eq!(action_container_inner.child_registries.len(), 1);
        assert_eq!(action_container_inner.root_registry.scope.priority, Action.priority());

        assert_eq!(step_container_inner.child_registries.len(), 0);
        assert_eq!(step_container_inner.root_registry.scope.priority, Step.priority());
        assert!(RcThreadSafety::ptr_eq(
            &step_container_inner.root_registry,
            &action_container_inner.child_registries[0]
        ));
    }

    #[tokio::test]
    #[traced_test]
    async fn test_scope_with_hierarchy() {
        let registry = RegistryBuilder::new()
            .provide_async(async || Ok(()), Runtime)
            .provide_async(async || Ok(((), ())), App)
            .provide_async(async || Ok(((), (), ())), Session)
            .provide_async(async || Ok(((), (), (), ())), Request)
            .provide_async(async || Ok(((), (), (), (), ())), Action)
            .provide_async(async || Ok(((), (), (), (), (), ())), Step);

        let runtime_container = Container::new_with_start_scope(registry, Runtime);
        let app_container = runtime_container.clone().enter().with_scope(App).build().unwrap();
        let session_container = runtime_container.clone().enter().with_scope(Session).build().unwrap();
        let request_container = app_container.clone().enter().with_scope(Request).build().unwrap();
        let action_container = request_container.clone().enter().with_scope(Action).build().unwrap();
        let step_container = action_container.clone().enter().with_scope(Step).build().unwrap();

        let runtime_container_inner = runtime_container.inner;
        let app_container_inner = app_container.inner;
        let session_container_inner = session_container.inner;
        let request_container_inner = request_container.inner;
        let action_container_inner = action_container.inner;
        let step_container_inner = step_container.inner;

        assert!(runtime_container_inner.parent.is_none());
        assert_eq!(runtime_container_inner.child_registries.len(), 5);
        assert_eq!(runtime_container_inner.root_registry.scope.priority, Runtime.priority());
        assert!(RcThreadSafety::ptr_eq(
            &app_container_inner.root_registry,
            &runtime_container_inner.child_registries[0]
        ));

        assert_eq!(app_container_inner.child_registries.len(), 4);
        assert_eq!(app_container_inner.root_registry.scope.priority, App.priority());
        assert!(RcThreadSafety::ptr_eq(
            &session_container_inner.root_registry,
            &app_container_inner.child_registries[0]
        ));

        assert_eq!(session_container_inner.child_registries.len(), 3);
        assert_eq!(session_container_inner.root_registry.scope.priority, Session.priority());
        assert!(RcThreadSafety::ptr_eq(
            &request_container_inner.root_registry,
            &session_container_inner.child_registries[0]
        ));

        assert_eq!(request_container_inner.child_registries.len(), 2);
        assert_eq!(request_container_inner.root_registry.scope.priority, Request.priority());
        assert!(RcThreadSafety::ptr_eq(
            &action_container_inner.root_registry,
            &request_container_inner.child_registries[0]
        ));

        assert_eq!(action_container_inner.child_registries.len(), 1);
        assert_eq!(action_container_inner.root_registry.scope.priority, Action.priority());
        assert!(RcThreadSafety::ptr_eq(
            &step_container_inner.root_registry,
            &action_container_inner.child_registries[0]
        ));

        assert_eq!(step_container_inner.child_registries.len(), 0);
        assert_eq!(step_container_inner.root_registry.scope.priority, Step.priority());
    }

    #[tokio::test]
    #[traced_test]
    async fn test_close_for_unresolved() {
        let finalizer_1_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_2_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_3_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_4_request_call_count = RcThreadSafety::new(AtomicU8::new(0));

        let registry = RegistryBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(((), ())), App)
            .provide_async(async || Ok(((), (), ())), Session)
            .provide(|| Ok(((), (), (), ())), Request)
            .add_finalizer({
                let finalizer_1_request_call_count = finalizer_1_request_call_count.clone();
                move |_: RcThreadSafety<()>| {
                    finalizer_1_request_call_count.fetch_add(1, Ordering::SeqCst);
                }
            })
            .add_finalizer({
                let finalizer_2_request_call_count = finalizer_2_request_call_count.clone();
                move |_: RcThreadSafety<((), ())>| {
                    finalizer_2_request_call_count.fetch_add(1, Ordering::SeqCst);
                }
            })
            .add_async_finalizer({
                let finalizer_3_request_call_count = finalizer_3_request_call_count.clone();
                move |_: RcThreadSafety<((), (), ())>| {
                    let finalizer_3_request_call_count = finalizer_3_request_call_count.clone();
                    async move {
                        finalizer_3_request_call_count.fetch_add(1, Ordering::SeqCst);
                    }
                }
            })
            .add_finalizer({
                let finalizer_4_request_call_count = finalizer_4_request_call_count.clone();
                move |_: RcThreadSafety<((), (), (), ())>| {
                    finalizer_4_request_call_count.fetch_add(1, Ordering::SeqCst);
                }
            });

        let app_container = Container::new(registry);
        let session_container = app_container.clone().enter().with_scope(Session).build().unwrap();
        let request_container = app_container.clone().enter().with_scope(Request).build().unwrap();

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
        let request_call_count = RcThreadSafety::new(AtomicU8::new(0));

        let finalizer_1_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_1_request_call_position = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_2_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_2_request_call_position = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_3_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_3_request_call_position = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_4_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_4_request_call_position = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_5_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_5_request_call_position = RcThreadSafety::new(AtomicU8::new(0));

        let registry = RegistryBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(((), ())), App)
            .provide_async(async || Ok(((), (), ())), App)
            .provide(|| Ok(((), (), (), ())), Request)
            .provide(|| Ok(((), (), (), (), ())), Request)
            .add_finalizer({
                let request_call_count = request_call_count.clone();
                let finalizer_1_request_call_position = finalizer_1_request_call_position.clone();
                let finalizer_1_request_call_count = finalizer_1_request_call_count.clone();
                move |_: RcThreadSafety<()>| {
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
                move |_: RcThreadSafety<((), ())>| {
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
                move |_: RcThreadSafety<((), (), ())>| {
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
                move |_: RcThreadSafety<((), (), (), ())>| {
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
                move |_: RcThreadSafety<((), (), (), (), ())>| {
                    request_call_count.fetch_add(1, Ordering::SeqCst);
                    finalizer_5_request_call_position.store(request_call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                    finalizer_5_request_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Finalizer 5 called");
                }
            });

        let app_container = Container::new(registry);
        let request_container = app_container.clone().enter().with_scope(Request).build().unwrap();

        let _ = request_container.get::<()>().await.unwrap();
        let _ = request_container.get::<((), ())>().await.unwrap();
        let _ = request_container.get::<((), (), ())>().await.unwrap();
        let _ = request_container.get::<((), (), (), (), ())>().await.unwrap();
        let _ = request_container.get::<((), (), (), ())>().await.unwrap();

        let runtime_container_resolved_set_count = app_container
            .inner
            .parent
            .as_ref()
            .unwrap()
            .sync
            .inner
            .cache
            .lock()
            .resolved
            .0
            .len();
        let app_container_resolved_set_count =
            app_container.sync.inner.cache.lock().resolved.0.len() + app_container.inner.cache.lock().resolved.0.len();
        let request_container_resolved_set_count = request_container.sync.inner.cache.lock().resolved.0.len();

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
        let call_count = RcThreadSafety::new(AtomicU8::new(0));

        let drop_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let drop_call_position = RcThreadSafety::new(AtomicU8::new(0));
        let instantiator_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let instantiator_call_position = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_1_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_1_call_position = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_2_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_2_call_position = RcThreadSafety::new(AtomicU8::new(0));

        struct Type1;
        struct Type2(RcThreadSafety<Type1>);

        struct DropWrapper<T> {
            val: T,
            call_count: RcThreadSafety<AtomicU8>,
            drop_call_count: RcThreadSafety<AtomicU8>,
            drop_call_position: RcThreadSafety<AtomicU8>,
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

        let registry = RegistryBuilder::new()
            .provide(|| Ok(Type1), App)
            .provide(|Inject(type_1): Inject<Type1>| Ok(Type2(type_1)), Request)
            .add_finalizer({
                let call_count = call_count.clone();
                let finalizer_1_call_count = finalizer_1_call_count.clone();
                let finalizer_1_call_position = finalizer_1_call_position.clone();
                move |_: RcThreadSafety<Type1>| {
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
                move |_: RcThreadSafety<Type2>| {
                    call_count.fetch_add(1, Ordering::SeqCst);
                    finalizer_2_call_position.store(call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                    finalizer_2_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Finalizer 2 called");
                }
            });

        let app_container = Container::new(registry);
        let request_container = app_container.enter_build().unwrap();
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

    #[tokio::test]
    #[traced_test]
    async fn test_async_close_on_drop() {
        let call_count = RcThreadSafety::new(AtomicU8::new(0));

        let drop_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let drop_call_position = RcThreadSafety::new(AtomicU8::new(0));
        let instantiator_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let instantiator_call_position = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_1_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_1_call_position = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_2_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_2_call_position = RcThreadSafety::new(AtomicU8::new(0));

        struct Type1;
        struct Type2(RcThreadSafety<Type1>);

        struct DropWrapper<T> {
            val: T,
            call_count: RcThreadSafety<AtomicU8>,
            drop_call_count: RcThreadSafety<AtomicU8>,
            drop_call_position: RcThreadSafety<AtomicU8>,
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

        let registry = RegistryBuilder::new()
            .provide(|| Ok(Type1), App)
            .provide(|Inject(type_1): Inject<Type1>| Ok(Type2(type_1)), Request)
            .add_async_finalizer({
                let call_count = call_count.clone();
                let finalizer_1_call_count = finalizer_1_call_count.clone();
                let finalizer_1_call_position = finalizer_1_call_position.clone();
                move |_: RcThreadSafety<Type1>| {
                    let call_count = call_count.clone();
                    let finalizer_1_call_count = finalizer_1_call_count.clone();
                    let finalizer_1_call_position = finalizer_1_call_position.clone();
                    async move {
                        call_count.fetch_add(1, Ordering::SeqCst);
                        finalizer_1_call_position.store(call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                        finalizer_1_call_count.fetch_add(1, Ordering::SeqCst);

                        debug!("Finalizer 1 called");
                    }
                }
            })
            .add_async_finalizer({
                let call_count = call_count.clone();
                let finalizer_2_call_count = finalizer_2_call_count.clone();
                let finalizer_2_call_position = finalizer_2_call_position.clone();
                move |_: RcThreadSafety<Type2>| {
                    let call_count = call_count.clone();
                    let finalizer_2_call_count = finalizer_2_call_count.clone();
                    let finalizer_2_call_position = finalizer_2_call_position.clone();
                    async move {
                        call_count.fetch_add(1, Ordering::SeqCst);
                        finalizer_2_call_position.store(call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                        finalizer_2_call_count.fetch_add(1, Ordering::SeqCst);

                        debug!("Finalizer 2 called");
                    }
                }
            });

        let app_container = Container::new(registry);
        let request_container = app_container.enter_build().unwrap();
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

        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_1_call_count.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_1_call_position.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_2_call_count.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_2_call_position.load(Ordering::SeqCst), 0);
        assert_eq!(instantiator_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(instantiator_call_position.load(Ordering::SeqCst), 2);
        assert_eq!(drop_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(drop_call_position.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_bounds() {
        fn impl_bounds<T: Send + 'static>() {}

        impl_bounds::<(Container, ContainerInner)>();
    }
}
