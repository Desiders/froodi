use alloc::{boxed::Box, vec::Vec};
use core::future::Future;
use parking_lot::Mutex;
use tracing::{debug, error, info_span, Instrument};

use super::{
    registry::{InstantiatorData, Registry},
    service::Service as _,
};
use crate::{
    any::TypeInfo,
    async_impl::registry::RegistryWithSync,
    cache::{Cache, Resolved},
    container::{BoxedContainerInner as BoxedSyncContainerInner, Container as SyncContainer, ContainerInner as SyncContainerInner},
    context::Context,
    errors::{InstantiatorErrorKind, ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind},
    registry::Registry as SyncRegistry,
    scope::{Scope, ScopeData, ScopeDataWithChildScopesData},
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
    #[must_use]
    pub fn new(RegistryWithSync { registry, sync }: RegistryWithSync) -> Self {
        let scope_with_child_scopes = registry.get_scope_with_child_scopes();
        let registry = RcThreadSafety::new(registry);
        let sync_registry = RcThreadSafety::new(sync);

        let mut sync_container = BoxedSyncContainerInner {
            cache: Cache::new(),
            context: Context::new(),
            registry: sync_registry.clone(),
            scope_data: scope_with_child_scopes.scope_data.expect("scopes len (is 0) should be > 0"),
            child_scopes_data: scope_with_child_scopes.child_scopes_data.clone(),
            parent: None,
            close_parent: false,
        };
        let mut container = BoxedContainerInner {
            cache: Cache::new(),
            context: Context::new(),
            registry: registry.clone(),
            scope_data: scope_with_child_scopes.scope_data.unwrap(),
            child_scopes_data: scope_with_child_scopes.child_scopes_data.clone(),
            parent: None,
            close_parent: false,
        };

        let mut child = scope_with_child_scopes.child();
        let mut scope_data = child.scope_data.expect("scopes len (is 1) should be > 1");

        let mut search_next = container.scope_data.is_skipped_by_default;
        while search_next {
            search_next = scope_data.is_skipped_by_default;

            sync_container = sync_container.init_child(sync_registry.clone(), scope_data, child.child_scopes_data.clone(), true);
            container = container.init_child(registry.clone(), scope_data, child.child_scopes_data.clone(), true);
            if search_next {
                child = child.child();
                scope_data = child.scope_data.expect("last scope can't be skipped by default");
            } else {
                break;
            }
        }

        (container, sync_container).into()
    }

    /// Creates container with start scope
    /// # Panics
    /// - Panics if registries builder doesn't create any registry.
    ///   This can occur if scopes are empty.
    /// - Panics if specified start scope not found in scopes.
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new_with_start_scope<S: Scope + Clone>(RegistryWithSync { registry, sync }: RegistryWithSync, scope: S) -> Self {
        let scope_with_child_scopes = registry.get_scope_with_child_scopes();
        let registry = RcThreadSafety::new(registry);
        let sync_registry = RcThreadSafety::new(sync);

        let mut sync_container = BoxedSyncContainerInner {
            cache: Cache::new(),
            context: Context::new(),
            registry: sync_registry.clone(),
            scope_data: scope_with_child_scopes.scope_data.expect("scopes len (is 0) should be > 0"),
            child_scopes_data: scope_with_child_scopes.child_scopes_data.clone(),
            parent: None,
            close_parent: false,
        };
        let mut container = BoxedContainerInner {
            cache: Cache::new(),
            context: Context::new(),
            registry: registry.clone(),
            scope_data: scope_with_child_scopes.scope_data.unwrap(),
            child_scopes_data: scope_with_child_scopes.child_scopes_data.clone(),
            parent: None,
            close_parent: false,
        };

        let priority = scope.priority();
        if container.scope_data.priority == priority {
            return (container, sync_container).into();
        }

        let mut child = scope_with_child_scopes.child();
        let mut scope_data = child.scope_data.expect("last scope can't be with another priority");

        let mut search_next = container.scope_data.priority != priority;
        while search_next {
            search_next = scope_data.priority != priority;

            sync_container = sync_container.init_child(sync_registry.clone(), scope_data, child.child_scopes_data.clone(), true);
            container = container.init_child(registry.clone(), scope_data, child.child_scopes_data.clone(), true);
            if search_next {
                child = child.child();
                scope_data = child.scope_data.expect("last scope can't be skipped by default");
            } else {
                break;
            }
        }

        (container, sync_container).into()
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
    #[allow(clippy::missing_errors_doc, clippy::multiple_bound_locations, clippy::missing_panics_doc)]
    pub fn get<Dep: SendSafety + SyncSafety + 'static>(
        &self,
    ) -> impl Future<Output = Result<RcThreadSafety<Dep>, ResolveErrorKind>> + SendSafety + '_ {
        let type_info = TypeInfo::of::<Dep>();

        Box::pin(
            async move {
                if let Some(dependency) = self.inner.cache.lock().get(&type_info) {
                    debug!("Found in cache");
                    return Ok(dependency);
                }
                debug!("Not found in cache");

                let Some(InstantiatorData {
                    instantiator,
                    finalizer,
                    config,
                    scope_data,
                    ..
                }) = self.inner.registry.get(&type_info)
                else {
                    debug!("No instantiator found, trying sync container");
                    return self.sync.get();
                };

                if self.inner.scope_data.priority > scope_data.priority {
                    let mut parent = self.inner.parent.as_ref().unwrap();
                    loop {
                        if parent.scope_data.priority == scope_data.priority {
                            return match (Self {
                                inner: parent.clone(),
                                sync: self.sync.clone(),
                            })
                            .get::<Dep>()
                            .await
                            {
                                Ok(dependency) => {
                                    self.inner.cache.lock().insert_rc(type_info, dependency.clone());
                                    Ok(dependency)
                                }
                                Err(err) => Err(err),
                            };
                        }
                        parent = parent.parent.as_ref().unwrap();
                    }
                }
                if scope_data.priority > self.inner.scope_data.priority {
                    let err = ResolveErrorKind::NoAccessible {
                        expected_scope_data: *scope_data,
                        actual_scope_data: self.inner.scope_data,
                    };
                    error!("{}", err);
                    return Err(err);
                }

                match instantiator.clone().call(self.clone()).await {
                    Ok(dependency) => match dependency.downcast::<Dep>() {
                        Ok(dependency) => {
                            let dependency = RcThreadSafety::new(*dependency);
                            let mut guard = self.inner.cache.lock();
                            if config.cache_provides {
                                guard.insert_rc(type_info, dependency.clone());
                                debug!("Cached");
                            }
                            if finalizer.is_some() {
                                guard.push_resolved(Resolved {
                                    type_info,
                                    dependency: dependency.clone(),
                                });
                                debug!("Pushed to resolved set");
                            }
                            Ok(dependency)
                        }
                        Err(incorrect_type) => {
                            let err = ResolveErrorKind::IncorrectType {
                                expected: type_info,
                                actual: TypeInfo::of_val(&*incorrect_type),
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
            .instrument(info_span!(
                "get",
                dependency = type_info.short_name(),
                scope = self.inner.scope_data.name,
            )),
        )
    }

    /// Gets a transient dependency from the container
    ///
    /// # Notes
    /// This method resolves a new instance of the dependency each time it is called,
    /// so it should be used for dependencies that are not cached or shared, and without finalizer.
    #[allow(clippy::missing_errors_doc, clippy::multiple_bound_locations, clippy::missing_panics_doc)]
    pub fn get_transient<Dep: 'static>(&self) -> impl Future<Output = Result<Dep, ResolveErrorKind>> + SendSafety + '_ {
        let type_info = TypeInfo::of::<Dep>();

        Box::pin(
            async move {
                let Some(InstantiatorData {
                    instantiator, scope_data, ..
                }) = self.inner.registry.get(&type_info)
                else {
                    debug!("No instantiator found, trying sync container");
                    return self.sync.get_transient();
                };

                if self.inner.scope_data.priority > scope_data.priority {
                    let mut parent = self.inner.parent.as_ref().unwrap();
                    loop {
                        if parent.scope_data.priority == scope_data.priority {
                            return (Self {
                                inner: parent.clone(),
                                sync: self.sync.clone(),
                            })
                            .get_transient()
                            .await;
                        }
                        parent = parent.parent.as_ref().unwrap();
                    }
                }
                if scope_data.priority > self.inner.scope_data.priority {
                    let err = ResolveErrorKind::NoAccessible {
                        expected_scope_data: *scope_data,
                        actual_scope_data: self.inner.scope_data,
                    };
                    error!("{}", err);
                    return Err(err);
                }

                match instantiator.clone().call(self.clone()).await {
                    Ok(dependency) => match dependency.downcast::<Dep>() {
                        Ok(dependency) => Ok(*dependency),
                        Err(incorrect_type) => {
                            let err = ResolveErrorKind::IncorrectType {
                                expected: type_info,
                                actual: TypeInfo::of_val(&*incorrect_type),
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
            .instrument(info_span!(
                "`get_transient",
                dependency = type_info.short_name(),
                scope = self.inner.scope_data.name,
            )),
        )
    }

    /// Closes the container, calling finalizers for resolved dependencies in LIFO order.
    ///
    /// # Warning
    /// This method can be called multiple times, but it will only call finalizers for dependencies that were resolved since the last call
    pub fn close(&self) -> impl Future<Output = ()> + SendSafety + '_ {
        Box::pin(async move {
            self.inner.close().await;
            self.sync.inner.close_with_parent_flag(false);

            let mut inner_parent = self.inner.parent.as_ref();
            let mut sync_parent = self.sync.inner.parent.as_ref();

            let mut close_parent = self.inner.close_parent;
            while close_parent {
                match (inner_parent, sync_parent) {
                    (Some(container), Some(sync_container)) => {
                        sync_container.inner.close_with_parent_flag(false);
                        container.close().await;

                        close_parent = container.close_parent;

                        inner_parent = container.parent.as_ref();
                        sync_parent = sync_container.inner.parent.as_ref();
                    }
                    (None, None) => break,
                    _ => unreachable!(),
                }
            }
        })
    }
}

impl Container {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    fn init_child_with_context(
        self,
        sync_container: SyncContainer,
        context: Context,
        registry: RcThreadSafety<Registry>,
        sync_registry: RcThreadSafety<SyncRegistry>,
        scope_data: ScopeData,
        child_scopes_data: Vec<ScopeData>,
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
                registry,
                scope_data,
                child_scopes_data: child_scopes_data.clone(),
                parent: Some(self.inner),
                close_parent,
            }),
            sync: SyncContainer {
                inner: RcThreadSafety::new(SyncContainerInner {
                    cache: Mutex::new(sync_cache),
                    context: Mutex::new(context),
                    registry: sync_registry,
                    scope_data,
                    child_scopes_data,
                    parent: Some(sync_container),
                    close_parent,
                }),
            },
        }
    }

    #[must_use]
    fn init_child(
        self,
        sync_container: SyncContainer,
        registry: RcThreadSafety<Registry>,
        sync_registry: RcThreadSafety<SyncRegistry>,
        scope_data: ScopeData,
        child_scopes_data: Vec<ScopeData>,
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
                registry,
                scope_data,
                child_scopes_data: child_scopes_data.clone(),
                parent: Some(self.inner),
                close_parent,
            }),
            sync: SyncContainer {
                inner: RcThreadSafety::new(SyncContainerInner {
                    cache: Mutex::new(sync_cache),
                    context: Mutex::new(sync_context),
                    registry: sync_registry,
                    scope_data,
                    child_scopes_data,
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

        let scope_with_child_scopes = self.container.inner.get_scope_with_child_scopes().child();
        let registry = self.container.inner.registry.clone();
        let sync_registry = self.container.sync.inner.registry.clone();
        let sync_container = self.container.sync.clone();

        let mut child = self.container.init_child(
            sync_container,
            registry,
            sync_registry,
            scope_with_child_scopes.scope_data.ok_or(NoChildRegistries)?,
            scope_with_child_scopes.child_scopes_data,
            false,
        );
        while child.inner.scope_data.is_skipped_by_default {
            let scope_with_child_scopes = child.inner.get_scope_with_child_scopes().child();
            let registry = child.inner.registry.clone();
            let sync_registry = child.sync.inner.registry.clone();
            let sync_container = child.sync.clone();

            child = child.init_child(
                sync_container,
                registry,
                sync_registry,
                scope_with_child_scopes.scope_data.ok_or(NoNonSkippedRegistries)?,
                scope_with_child_scopes.child_scopes_data,
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

        let scope_with_child_scopes = self.container.inner.get_scope_with_child_scopes().child();
        let registry = self.container.inner.registry.clone();
        let sync_registry = self.container.sync.inner.registry.clone();
        let sync_container = self.container.sync.clone();
        let priority = self.scope.priority();

        let mut child = self.container.init_child(
            sync_container,
            registry,
            sync_registry,
            scope_with_child_scopes.scope_data.ok_or(NoChildRegistries)?,
            scope_with_child_scopes.child_scopes_data,
            false,
        );
        while child.inner.scope_data.priority != priority {
            let scope_with_child_scopes = child.inner.get_scope_with_child_scopes().child();
            let registry = child.inner.registry.clone();
            let sync_registry = child.sync.inner.registry.clone();
            let sync_container = child.sync.clone();

            child = child.init_child(
                sync_container,
                registry,
                sync_registry,
                scope_with_child_scopes.scope_data.ok_or(NoChildRegistriesWithScope {
                    name: self.scope.name(),
                    priority,
                })?,
                scope_with_child_scopes.child_scopes_data,
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

        let scope_with_child_scopes = self.container.inner.get_scope_with_child_scopes().child();
        let context = self.context.clone();
        let registry = self.container.inner.registry.clone();
        let sync_registry = self.container.sync.inner.registry.clone();
        let sync_container = self.container.sync.clone();

        let mut child = self.container.init_child_with_context(
            sync_container,
            context,
            registry,
            sync_registry,
            scope_with_child_scopes.scope_data.ok_or(NoChildRegistries)?,
            scope_with_child_scopes.child_scopes_data,
            false,
        );
        while child.inner.scope_data.is_skipped_by_default {
            let scope_with_child_scopes = child.inner.get_scope_with_child_scopes().child();
            let context = self.context.clone();
            let registry = child.inner.registry.clone();
            let sync_registry = child.sync.inner.registry.clone();
            let sync_container = child.sync.clone();

            child = child.init_child_with_context(
                sync_container,
                context,
                registry,
                sync_registry,
                scope_with_child_scopes.scope_data.ok_or(NoNonSkippedRegistries)?,
                scope_with_child_scopes.child_scopes_data,
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

        let scope_with_child_scopes = self.container.inner.get_scope_with_child_scopes().child();
        let context = self.context.clone();
        let registry = self.container.inner.registry.clone();
        let sync_registry = self.container.sync.inner.registry.clone();
        let sync_container = self.container.sync.clone();
        let priority = self.scope.priority();

        let mut child = self.container.init_child_with_context(
            sync_container,
            context,
            registry,
            sync_registry,
            scope_with_child_scopes.scope_data.ok_or(NoChildRegistries)?,
            scope_with_child_scopes.child_scopes_data,
            false,
        );
        while child.inner.scope_data.priority != priority {
            let scope_with_child_scopes = child.inner.get_scope_with_child_scopes().child();
            let context = self.context.clone();
            let registry = child.inner.registry.clone();
            let sync_registry = child.sync.inner.registry.clone();
            let sync_container = child.sync.clone();

            child = child.init_child_with_context(
                sync_container,
                context,
                registry,
                sync_registry,
                scope_with_child_scopes.scope_data.ok_or(NoChildRegistriesWithScope {
                    name: self.scope.name(),
                    priority,
                })?,
                scope_with_child_scopes.child_scopes_data,
                true,
            );
        }

        Ok(child)
    }
}

struct BoxedContainerInner {
    cache: Cache,
    context: Context,
    registry: RcThreadSafety<Registry>,
    scope_data: ScopeData,
    child_scopes_data: Vec<ScopeData>,
    parent: Option<Box<BoxedContainerInner>>,
    close_parent: bool,
}

impl BoxedContainerInner {
    #[must_use]
    fn init_child(
        self,
        registry: RcThreadSafety<Registry>,
        scope_data: ScopeData,
        child_scopes_data: Vec<ScopeData>,
        close_parent: bool,
    ) -> Self {
        let mut cache = self.cache.child();
        let context = self.context.clone();
        cache.append_context(&mut context.clone());

        Self {
            cache,
            context,
            registry,
            scope_data,
            child_scopes_data,
            parent: Some(Box::new(self)),
            close_parent,
        }
    }
}

impl From<BoxedContainerInner> for ContainerInner {
    fn from(
        BoxedContainerInner {
            cache,
            context,
            registry,
            scope_data,
            child_scopes_data,
            parent,
            close_parent,
        }: BoxedContainerInner,
    ) -> Self {
        Self {
            cache: Mutex::new(cache),
            context: Mutex::new(context),
            registry,
            scope_data,
            child_scopes_data,
            parent: parent.map(|parent| RcThreadSafety::new((*parent).into())),
            close_parent,
        }
    }
}

impl From<(BoxedContainerInner, BoxedSyncContainerInner)> for Container {
    fn from((inner, sync): (BoxedContainerInner, BoxedSyncContainerInner)) -> Self {
        Self {
            inner: RcThreadSafety::new(inner.into()),
            sync: sync.into(),
        }
    }
}

struct ContainerInner {
    cache: Mutex<Cache>,
    context: Mutex<Context>,
    registry: RcThreadSafety<Registry>,
    scope_data: ScopeData,
    child_scopes_data: Vec<ScopeData>,
    parent: Option<RcThreadSafety<ContainerInner>>,
    close_parent: bool,
}

impl ContainerInner {
    #[inline]
    pub(crate) fn get_scope_with_child_scopes(&self) -> ScopeDataWithChildScopesData {
        ScopeDataWithChildScopesData::new(self.scope_data, self.child_scopes_data.clone())
    }

    #[allow(clippy::missing_panics_doc)]
    fn close(&self) -> impl Future<Output = ()> + SendSafety + '_ {
        let mut resolved_set = self.cache.lock().take_resolved_set();

        Box::pin(async move {
            while let Some(Resolved { type_info, dependency }) = resolved_set.0.pop_back() {
                let InstantiatorData { finalizer, .. } = self
                    .registry
                    .get(&type_info)
                    .expect("Instantiator should be present for resolved type");

                if let Some(finalizer) = finalizer {
                    let _ = finalizer.clone().call(dependency).await;
                    debug!(?type_info, "Finalizer called");
                }
            }

            // We need to clear cache and fill it with the context as in start of the container usage
            #[allow(clippy::assigning_clones)]
            {
                self.cache.lock().map = self.context.lock().map.clone();
            }
        })
    }
}

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    extern crate std;

    use super::{Container, ContainerInner};
    use crate::{
        async_registry, registry, scope::DefaultScope::*, utils::thread_safety::RcThreadSafety, Inject, InjectTransient, ResolveErrorKind,
        Scope,
    };

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

        let app_container = Container::new(async_registry! {
            scope(Runtime) [
                provide(async || Ok(CAAAAA)),
            ],
            scope(App) [
                provide(async |Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa))),
                provide(async || Ok(B(2))),
            ],
            scope(Session) [
                provide(async |Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa))),
            ],
            scope(Request) [
                provide(async |Inject(caaa): Inject<CAAA>| Ok(CAA(caaa))),
                provide(async |Inject(caa): Inject<CAA>| Ok(CA(caa))),
            ],
            scope(Action) [
                provide(async |Inject(ca): Inject<CA>| Ok(C(ca))),
            ],
            scope(Step) [
                provide(async |Inject(b): Inject<B>, Inject(c): Inject<C>| Ok(A(b, c))),
            ],
        });
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

        let app_container = Container::new(async_registry! {
            scope(Runtime) [
                provide(async || Ok(CAAAAA)),
            ],
            scope(App) [
                provide(async |Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa))),
                provide(async || Ok(B(2))),
            ],
            scope(Session) [
                provide(async |Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa))),
            ],
            scope(Request) [
                provide(async |Inject(caaa): Inject<CAAA>| Ok(CAA(caaa))),
                provide(async |Inject(caa): Inject<CAA>| Ok(CA(caa))),
            ],
            scope(Action) [
                provide(async |Inject(ca): Inject<CA>| Ok(C(ca))),
            ],
            scope(Step) [
                provide(async |Inject(b): Inject<B>, Inject(c): Inject<C>| Ok(A(b, c))),
            ],
        });
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
        let app_container = Container::new(async_registry! {
            extend(registry! {
                scope(App) [
                    provide(|| Ok(RequestTransient1)),
                ],
                scope(Request) [
                    provide(|InjectTransient(req): InjectTransient<RequestTransient1>| Ok(RequestTransient2(req))),
                    provide(|InjectTransient(req_1): InjectTransient<RequestTransient1>, InjectTransient(req_2): InjectTransient<RequestTransient2>| {
                        Ok(RequestTransient3(req_1, req_2))
                    }),
                ]
            })
        });
        let request_container = app_container.clone().enter().with_scope(Request).build().unwrap();

        assert!(app_container.get_transient::<RequestTransient1>().await.is_ok());
        assert!(matches!(
            app_container.get_transient::<RequestTransient2>().await,
            Err(ResolveErrorKind::NoAccessible {
                expected_scope_data: _,
                actual_scope_data: _,
            }),
        ));
        assert!(matches!(
            app_container.get_transient::<RequestTransient3>().await,
            Err(ResolveErrorKind::NoAccessible {
                expected_scope_data: _,
                actual_scope_data: _,
            }),
        ));

        assert!(request_container.get_transient::<RequestTransient1>().await.is_ok());
        assert!(request_container.get_transient::<RequestTransient2>().await.is_ok());
        assert!(request_container.get_transient::<RequestTransient3>().await.is_ok());
    }

    #[tokio::test]
    #[traced_test]
    async fn test_async_transient_get() {
        let app_container = Container::new(async_registry! {
            scope(App) [
                provide(async || Ok(RequestTransient1)),
            ],
            scope(Request) [
                provide(async |InjectTransient(req): InjectTransient<RequestTransient1>| Ok(RequestTransient2(req))),
                provide(async |InjectTransient(req_1): InjectTransient<RequestTransient1>, InjectTransient(req_2): InjectTransient<RequestTransient2>| {
                    Ok(RequestTransient3(req_1, req_2))
                }),
            ]
        });
        let request_container = app_container.clone().enter().with_scope(Request).build().unwrap();

        assert!(app_container.get_transient::<RequestTransient1>().await.is_ok());
        assert!(matches!(
            app_container.get_transient::<RequestTransient2>().await,
            Err(ResolveErrorKind::NoAccessible {
                expected_scope_data: _,
                actual_scope_data: _,
            }),
        ));
        assert!(matches!(
            app_container.get_transient::<RequestTransient3>().await,
            Err(ResolveErrorKind::NoAccessible {
                expected_scope_data: _,
                actual_scope_data: _,
            }),
        ));

        assert!(request_container.get_transient::<RequestTransient1>().await.is_ok());
        assert!(request_container.get_transient::<RequestTransient2>().await.is_ok());
        assert!(request_container.get_transient::<RequestTransient3>().await.is_ok());
    }

    #[tokio::test]
    #[traced_test]
    async fn test_scope_hierarchy() {
        let app_container = Container::new(async_registry! {
            scope(Runtime) [
                provide(async || Ok(())),
            ],
            scope(App) [
                provide(async || Ok(((), ()))),
            ],
            scope(Session) [
                provide(async || Ok(((), (), ()))),
            ],
            scope(Request) [
                provide(async || Ok(((), (), (), ()))),
            ],
            scope(Action) [
                provide(async || Ok(((), (), (), (), ()))),
            ],
            scope(Step) [
                provide(async || Ok(((), (), (), (), (), ()))),
            ],
        });
        let request_container = app_container.clone().enter_build().unwrap();
        let action_container = request_container.clone().enter_build().unwrap();
        let step_container = action_container.clone().enter_build().unwrap();

        let app_container_inner = app_container.inner;
        let request_container_inner = request_container.inner;
        let action_container_inner = action_container.inner;
        let step_container_inner = step_container.inner;

        // Runtime scope is skipped by default, but it is still present in the parent
        assert_eq!(app_container_inner.parent.as_ref().unwrap().scope_data.priority, Runtime.priority());
        assert_eq!(app_container_inner.child_scopes_data.len(), 4);
        assert_eq!(app_container_inner.scope_data.priority, App.priority());

        // Session scope is skipped by default, but it is still present in the child registries
        assert_eq!(
            request_container_inner.parent.as_ref().unwrap().scope_data.priority,
            Session.priority()
        );
        assert_eq!(request_container_inner.child_scopes_data.len(), 2);
        assert_eq!(request_container_inner.scope_data.priority, Request.priority());
        // Session scope is skipped by default, so it is not the first child registry
        assert_eq!(
            request_container_inner.scope_data.priority,
            app_container_inner.child_scopes_data[1].priority
        );
        assert_eq!(
            action_container_inner.scope_data.priority,
            request_container_inner.child_scopes_data[0].priority
        );

        assert_eq!(action_container_inner.child_scopes_data.len(), 1);
        assert_eq!(action_container_inner.scope_data.priority, Action.priority());

        assert_eq!(step_container_inner.child_scopes_data.len(), 0);
        assert_eq!(step_container_inner.scope_data.priority, Step.priority());
        assert_eq!(
            step_container_inner.scope_data.priority,
            action_container_inner.child_scopes_data[0].priority
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn test_scope_with_hierarchy() {
        let runtime_container = Container::new_with_start_scope(
            async_registry! {
                scope(Runtime) [
                    provide(async || Ok(())),
                ],
                scope(App) [
                    provide(async || Ok(((), ()))),
                ],
                scope(Session) [
                    provide(async || Ok(((), (), ()))),
                ],
                scope(Request) [
                    provide(async || Ok(((), (), (), ()))),
                ],
                scope(Action) [
                    provide(async || Ok(((), (), (), (), ()))),
                ],
                scope(Step) [
                    provide(async || Ok(((), (), (), (), (), ()))),
                ],
            },
            Runtime,
        );
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
        assert_eq!(runtime_container_inner.child_scopes_data.len(), 5);
        assert_eq!(runtime_container_inner.scope_data.priority, Runtime.priority());
        assert_eq!(
            app_container_inner.scope_data.priority,
            runtime_container_inner.child_scopes_data[0].priority
        );

        assert_eq!(app_container_inner.child_scopes_data.len(), 4);
        assert_eq!(app_container_inner.scope_data.priority, App.priority());
        assert_eq!(
            session_container_inner.scope_data.priority,
            app_container_inner.child_scopes_data[0].priority
        );

        assert_eq!(session_container_inner.child_scopes_data.len(), 3);
        assert_eq!(session_container_inner.scope_data.priority, Session.priority());
        assert_eq!(
            request_container_inner.scope_data.priority,
            session_container_inner.child_scopes_data[0].priority
        );

        assert_eq!(request_container_inner.child_scopes_data.len(), 2);
        assert_eq!(request_container_inner.scope_data.priority, Request.priority());
        assert_eq!(
            action_container_inner.scope_data.priority,
            request_container_inner.child_scopes_data[0].priority
        );

        assert_eq!(action_container_inner.child_scopes_data.len(), 1);
        assert_eq!(action_container_inner.scope_data.priority, Action.priority());
        assert_eq!(
            step_container_inner.scope_data.priority,
            action_container_inner.child_scopes_data[0].priority
        );

        assert_eq!(step_container_inner.child_scopes_data.len(), 0);
        assert_eq!(step_container_inner.scope_data.priority, Step.priority());
    }

    #[tokio::test]
    #[traced_test]
    async fn test_close_for_unresolved() {
        let finalizer_1_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_2_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_3_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_4_request_call_count = RcThreadSafety::new(AtomicU8::new(0));

        let app_container = Container::new(async_registry! {
            scope(Session) [
                provide(
                    async || Ok(((), (), ())),
                    finalizer = {
                        let finalizer_3_request_call_count = finalizer_3_request_call_count.clone();
                        move |_: RcThreadSafety<((), (), ())>| {
                            let finalizer_3_request_call_count = finalizer_3_request_call_count.clone();
                            async move {
                                finalizer_3_request_call_count.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                    }
                ),
            ],
            extend(registry! {
                scope(Runtime) [
                    provide(
                        || Ok(()),
                        finalizer = {
                            let finalizer_1_request_call_count = finalizer_1_request_call_count.clone();
                            move |_: RcThreadSafety<()>| {
                                finalizer_1_request_call_count.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                    ),
                ],
                scope(App) [
                    provide(
                        || Ok(((), ())),
                        finalizer = {
                            let finalizer_2_request_call_count = finalizer_2_request_call_count.clone();
                            move |_: RcThreadSafety<((), ())>| {
                                finalizer_2_request_call_count.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                    ),
                ],
                scope(Request) [
                    provide(
                        || Ok(((), (), (), ())),
                        finalizer = {
                            let finalizer_4_request_call_count = finalizer_4_request_call_count.clone();
                            move |_: RcThreadSafety<((), (), (), ())>| {
                                finalizer_4_request_call_count.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                    ),
                ],
            }),
        });
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

        let app_container = Container::new(async_registry! {
            scope(App) [
                provide(
                    async || Ok(((), (), ())),
                    finalizer = {
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
                    }
                ),
            ],
            extend(registry! {
                scope(Runtime) [
                    provide(
                        || Ok(()),
                        finalizer = {
                            let request_call_count = request_call_count.clone();
                            let finalizer_1_request_call_position = finalizer_1_request_call_position.clone();
                            let finalizer_1_request_call_count = finalizer_1_request_call_count.clone();
                            move |_: RcThreadSafety<()>| {
                                request_call_count.fetch_add(1, Ordering::SeqCst);
                                finalizer_1_request_call_position.store(request_call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                                finalizer_1_request_call_count.fetch_add(1, Ordering::SeqCst);

                                debug!("Finalizer 1 called");
                            }
                        }
                    ),
                ],
                scope(App) [
                    provide(
                        || Ok(((), ())),
                        finalizer = {
                            let request_call_count = request_call_count.clone();
                            let finalizer_2_request_call_position = finalizer_2_request_call_position.clone();
                            let finalizer_2_request_call_count = finalizer_2_request_call_count.clone();
                            move |_: RcThreadSafety<((), ())>| {
                                request_call_count.fetch_add(1, Ordering::SeqCst);
                                finalizer_2_request_call_position.store(request_call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                                finalizer_2_request_call_count.fetch_add(1, Ordering::SeqCst);

                                debug!("Finalizer 2 called");
                            }
                        }
                    ),
                ],
                scope(Request) [
                    provide(
                        || Ok(((), (), (), ())),
                        finalizer = {
                            let request_call_count = request_call_count.clone();
                            let finalizer_4_request_call_position = finalizer_4_request_call_position.clone();
                            let finalizer_4_request_call_count = finalizer_4_request_call_count.clone();
                            move |_: RcThreadSafety<((), (), (), ())>| {
                                request_call_count.fetch_add(1, Ordering::SeqCst);
                                finalizer_4_request_call_position.store(request_call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                                finalizer_4_request_call_count.fetch_add(1, Ordering::SeqCst);

                                debug!("Finalizer 4 called");
                            }
                        }
                    ),
                    provide(
                        || Ok(((), (), (), (), ())),
                        finalizer = {
                            let request_call_count = request_call_count.clone();
                            let finalizer_5_request_call_position = finalizer_5_request_call_position.clone();
                            let finalizer_5_request_call_count = finalizer_5_request_call_count.clone();
                            move |_: RcThreadSafety<((), (), (), (), ())>| {
                                request_call_count.fetch_add(1, Ordering::SeqCst);
                                finalizer_5_request_call_position.store(request_call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                                finalizer_5_request_call_count.fetch_add(1, Ordering::SeqCst);

                                debug!("Finalizer 5 called");
                            }
                        }
                    ),
                ],
            })
        });
        let request_container = app_container.clone().enter().with_scope(Request).build().unwrap();

        let _ = request_container.get::<()>().await.unwrap();
        let _ = request_container.get::<((), ())>().await.unwrap();
        let _ = request_container.get::<((), (), ())>().await.unwrap();
        let _ = request_container.get::<((), (), (), (), ())>().await.unwrap();
        let _ = request_container.get::<((), (), (), ())>().await.unwrap();

        let runtime_container_resolved_set_count = app_container
            .sync
            .inner
            .parent
            .as_ref()
            .unwrap()
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

        let app_container = Container::new(async_registry! {
            extend(registry! {
                scope(App) [
                    provide(
                        || Ok(Type1),
                        finalizer = {
                            let call_count = call_count.clone();
                            let finalizer_1_call_count = finalizer_1_call_count.clone();
                            let finalizer_1_call_position = finalizer_1_call_position.clone();
                            move |_: RcThreadSafety<Type1>| {
                                call_count.fetch_add(1, Ordering::SeqCst);
                                finalizer_1_call_position.store(call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                                finalizer_1_call_count.fetch_add(1, Ordering::SeqCst);

                                debug!("Finalizer 1 called");
                            }
                        }
                    ),
                    provide(
                        |Inject(type_1): Inject<Type1>| Ok(Type2(type_1)),
                        finalizer = {
                            let call_count = call_count.clone();
                            let finalizer_2_call_count = finalizer_2_call_count.clone();
                            let finalizer_2_call_position = finalizer_2_call_position.clone();
                            move |_: RcThreadSafety<Type2>| {
                                call_count.fetch_add(1, Ordering::SeqCst);
                                finalizer_2_call_position.store(call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                                finalizer_2_call_count.fetch_add(1, Ordering::SeqCst);

                                debug!("Finalizer 2 called");
                            }
                        }
                    ),
                ]
            }),
        });
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

        let app_container = Container::new(async_registry! {
            scope(App) [
                provide(
                    async || Ok(Type1),
                    finalizer = {
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
                    }
                ),
                provide(
                    async |Inject(type_1): Inject<Type1>| Ok(Type2(type_1)),
                    finalizer = {
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
                    }
                ),
            ]
        });
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

    #[tokio::test]
    #[traced_test]
    async fn test_thread_safe() {
        struct Request1 {
            #[cfg(not(feature = "thread_safe"))]
            _phantom: core::marker::PhantomData<*const ()>,
        }

        fn impl_bounds<T: Send + Sync + 'static>() {}

        impl_bounds::<(Container, ContainerInner)>();

        let app_container = Container::new(async_registry! {
            scope(App) [
                provide(async || Ok(RequestTransient1)),
            ],
        });
        tokio::spawn(async move {
            let request1 = app_container.get_transient::<RequestTransient1>().await;
            let request2 = app_container.get::<Request1>().await;

            assert!(request1.is_ok());
            assert!(request2.is_ok());
        });
    }
}
