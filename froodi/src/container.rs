use alloc::{boxed::Box, vec::Vec};
use core::any::{type_name, TypeId};
use parking_lot::Mutex;
use tracing::{debug, error, info_span};

use super::cache::Cache;
use crate::{
    cache::Resolved,
    context::Context,
    errors::{InstantiatorErrorKind, ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind},
    registry::{InstantiatorData, Registry},
    scope::{Scope, ScopeData, ScopeDataWithChildScopesData},
    service::Service as _,
    utils::thread_safety::{RcThreadSafety, SendSafety, SyncSafety},
};

#[derive(Clone)]
pub struct Container {
    pub(crate) inner: RcThreadSafety<ContainerInner>,
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
    pub fn new(registry: Registry) -> Self {
        let scope_with_child_scopes = registry.get_scope_with_child_scopes();
        let registry = RcThreadSafety::new(registry);
        let mut container = BoxedContainerInner {
            cache: Cache::new(),
            context: Context::new(),
            registry: registry.clone(),
            scope_data: scope_with_child_scopes.scope_data.clone().expect("scopes len (is 0) should be > 0"),
            child_scopes_data: scope_with_child_scopes.child_scopes_data.clone(),
            parent: None,
            close_parent: false,
        };

        let mut child = scope_with_child_scopes.child();
        let mut scope_data = child.scope_data.clone().expect("scopes len (is 1) should be > 1");

        let mut search_next = container.scope_data.is_skipped_by_default;
        while search_next {
            search_next = scope_data.is_skipped_by_default;
            container = container.init_child(registry.clone(), scope_data, child.child_scopes_data.clone(), true);
            if search_next {
                child = child.child();
                scope_data = child.scope_data.expect("last scope can't be skipped by default");
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
    pub fn new_with_start_scope<S: Scope>(registry: Registry, scope: S) -> Self {
        let scope_with_child_scopes = registry.get_scope_with_child_scopes();
        let registry = RcThreadSafety::new(registry);
        let mut container = BoxedContainerInner {
            cache: Cache::new(),
            context: Context::new(),
            registry: registry.clone(),
            scope_data: scope_with_child_scopes.scope_data.clone().expect("scopes len (is 0) should be > 0"),
            child_scopes_data: scope_with_child_scopes.child_scopes_data.clone(),
            parent: None,
            close_parent: false,
        };

        let priority = scope.priority();
        if container.scope_data.priority == priority {
            return container.into();
        }

        let mut child = scope_with_child_scopes.child();
        let mut scope_data = child.scope_data.clone().expect("last scope can't be with another priority");

        let mut search_next = container.scope_data.priority != priority;
        while search_next {
            search_next = scope_data.priority != priority;
            container = container.init_child(registry.clone(), scope_data, child.child_scopes_data.clone(), true);
            if search_next {
                child = child.child();
                scope_data = child.scope_data.expect("last scope can't be with another priority");
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
    #[allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
    pub fn get<Dep: SendSafety + SyncSafety + 'static>(&self) -> Result<RcThreadSafety<Dep>, ResolveErrorKind> {
        let span = info_span!("get", dependency = type_name::<Dep>(), scope = self.inner.scope_data.name);
        let _guard = span.enter();

        let type_id = TypeId::of::<Dep>();

        if let Some(dependency) = self.inner.cache.lock().get(&type_id) {
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
        }) = self.inner.registry.get(&type_id)
        else {
            let err = ResolveErrorKind::NoInstantiator;
            error!("{}", err);
            return Err(err);
        };

        if self.inner.scope_data.priority > scope_data.priority {
            let mut parent = self.inner.parent.as_ref().unwrap();
            loop {
                if parent.inner.scope_data.priority == scope_data.priority {
                    return match parent.get::<Dep>() {
                        Ok(dependency) => {
                            self.inner.cache.lock().insert_rc(dependency.clone());
                            Ok(dependency)
                        }
                        Err(err) => Err(err),
                    };
                }
                parent = parent.inner.parent.as_ref().unwrap();
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

        match instantiator.clone().call(self.clone()) {
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
    ///
    /// # Warning
    /// Context isn't used here. To get dependencies from the context, use [`Self::get`]
    #[allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
    pub fn get_transient<Dep: 'static>(&self) -> Result<Dep, ResolveErrorKind> {
        let span = info_span!("get_transient", dependency = type_name::<Dep>(), scope = self.inner.scope_data.name);
        let _guard = span.enter();

        let type_id = TypeId::of::<Dep>();

        let Some(InstantiatorData {
            instantiator, scope_data, ..
        }) = self.inner.registry.get(&type_id)
        else {
            let err = ResolveErrorKind::NoInstantiator;
            error!("{}", err);
            return Err(err);
        };

        if self.inner.scope_data.priority > scope_data.priority {
            let mut parent = self.inner.parent.as_ref().unwrap();
            loop {
                if parent.inner.scope_data.priority == scope_data.priority {
                    return parent.get_transient();
                }
                parent = parent.inner.parent.as_ref().unwrap();
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

        match instantiator.clone().call(self.clone()) {
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
    pub fn close(&self) {
        self.inner.close();
    }
}

impl Container {
    #[inline]
    #[must_use]
    fn init_child_with_context(
        self,
        context: Context,
        registry: RcThreadSafety<Registry>,
        scope_data: ScopeData,
        child_scopes_data: Vec<ScopeData>,
        close_parent: bool,
    ) -> Container {
        let mut cache = self.inner.cache.lock().child();
        cache.append_context(&mut context.clone());

        Container {
            inner: RcThreadSafety::new(ContainerInner {
                cache: Mutex::new(cache),
                context: Mutex::new(context),
                registry,
                scope_data,
                child_scopes_data,
                parent: Some(self),
                close_parent,
            }),
        }
    }

    #[inline]
    #[must_use]
    fn init_child(
        self,
        registry: RcThreadSafety<Registry>,
        scope_data: ScopeData,
        child_scopes_data: Vec<ScopeData>,
        close_parent: bool,
    ) -> Container {
        let mut cache = self.inner.cache.lock().child();
        let context = self.inner.context.lock().clone();
        cache.append_context(&mut context.clone());

        Container {
            inner: RcThreadSafety::new(ContainerInner {
                cache: Mutex::new(cache),
                context: Mutex::new(context),
                registry,
                scope_data,
                child_scopes_data,
                parent: Some(self),
                close_parent,
            }),
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

        let mut child = self.container.init_child(
            registry,
            scope_with_child_scopes.scope_data.ok_or(NoChildRegistries)?,
            scope_with_child_scopes.child_scopes_data,
            false,
        );
        while child.inner.scope_data.is_skipped_by_default {
            let scope_with_child_scopes = child.inner.get_scope_with_child_scopes().child();
            let registry = child.inner.registry.clone();

            child = child.init_child(
                registry,
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
        let priority = self.scope.priority();

        let mut child = self.container.init_child(
            registry,
            scope_with_child_scopes.scope_data.ok_or(NoChildRegistries)?,
            scope_with_child_scopes.child_scopes_data,
            false,
        );
        while child.inner.scope_data.priority != priority {
            let scope_with_child_scopes = child.inner.get_scope_with_child_scopes().child();
            let registry = child.inner.registry.clone();

            child = child.init_child(
                registry,
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

        let mut child = self.container.init_child_with_context(
            context,
            registry,
            scope_with_child_scopes.scope_data.ok_or(NoChildRegistries)?,
            scope_with_child_scopes.child_scopes_data,
            false,
        );
        while child.inner.scope_data.is_skipped_by_default {
            let scope_with_child_scopes = child.inner.get_scope_with_child_scopes().child();
            let context = self.context.clone();
            let registry = child.inner.registry.clone();

            child = child.init_child_with_context(
                context,
                registry,
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
        let priority = self.scope.priority();

        let mut child = self.container.init_child_with_context(
            context,
            registry,
            scope_with_child_scopes.scope_data.ok_or(NoChildRegistries)?,
            scope_with_child_scopes.child_scopes_data,
            false,
        );
        while child.inner.scope_data.priority != priority {
            let scope_with_child_scopes = child.inner.get_scope_with_child_scopes().child();
            let context = self.context.clone();
            let registry = child.inner.registry.clone();

            child = child.init_child_with_context(
                context,
                registry,
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

#[derive(Clone)]
pub(crate) struct BoxedContainerInner {
    pub(crate) cache: Cache,
    pub(crate) context: Context,
    pub(crate) registry: RcThreadSafety<Registry>,
    pub(crate) scope_data: ScopeData,
    pub(crate) child_scopes_data: Vec<ScopeData>,
    pub(crate) parent: Option<Box<BoxedContainerInner>>,
    pub(crate) close_parent: bool,
}

impl BoxedContainerInner {
    #[inline]
    #[must_use]
    pub(crate) fn init_child(
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

impl From<BoxedContainerInner> for Container {
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
            inner: RcThreadSafety::new(ContainerInner {
                cache: Mutex::new(cache),
                context: Mutex::new(context),
                registry,
                scope_data,
                child_scopes_data,
                parent: parent.map(|parent| (*parent).into()),
                close_parent,
            }),
        }
    }
}

pub(crate) struct ContainerInner {
    pub(crate) cache: Mutex<Cache>,
    pub(crate) context: Mutex<Context>,
    pub(crate) registry: RcThreadSafety<Registry>,
    pub(crate) scope_data: ScopeData,
    pub(crate) child_scopes_data: Vec<ScopeData>,
    pub(crate) parent: Option<Container>,
    pub(crate) close_parent: bool,
}

impl ContainerInner {
    #[inline]
    pub(crate) fn get_scope_with_child_scopes(&self) -> ScopeDataWithChildScopesData {
        ScopeDataWithChildScopesData::new(self.scope_data, self.child_scopes_data.clone())
    }

    #[allow(clippy::missing_panics_doc)]
    fn close(&self) {
        self.close_with_parent_flag(self.close_parent);
    }

    pub(crate) fn close_with_parent_flag(&self, close_parent: bool) {
        let mut resolved_set = self.cache.lock().take_resolved_set();
        while let Some(Resolved { type_id, dependency }) = resolved_set.0.pop_back() {
            let InstantiatorData { finalizer, .. } = self
                .registry
                .get(&type_id)
                .expect("Instantiator should be present for resolved type");

            if let Some(finalizer) = finalizer {
                let _ = finalizer.clone().call(dependency);
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
                parent.close();
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

    use super::Container;
    use crate::{
        container::ContainerInner,
        inject::{Inject, InjectTransient},
        registry,
        scope::DefaultScope::*,
        utils::thread_safety::RcThreadSafety,
        ResolveErrorKind, Scope,
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

    #[test]
    #[traced_test]
    fn test_scoped_get() {
        struct A(RcThreadSafety<B>, RcThreadSafety<C>);
        struct B(i32);
        struct C(RcThreadSafety<CA>);
        struct CA(RcThreadSafety<CAA>);
        struct CAA(RcThreadSafety<CAAA>);
        struct CAAA(RcThreadSafety<CAAAA>);
        struct CAAAA(RcThreadSafety<CAAAAA>);
        struct CAAAAA;

        let app_container = Container::new(registry! {
            scope(Runtime) [
                provide(|| Ok(CAAAAA)),
            ],
            scope(App) [
                provide(|Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa))),
                provide(|| Ok(B(2))),
            ],
            scope(Session) [
                provide(|Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa))),
            ],
            scope(Request) [
                provide(|Inject(caaa): Inject<CAAA>| Ok(CAA(caaa))),
                provide(|Inject(caa): Inject<CAA>| Ok(CA(caa))),
            ],
            scope(Action) [
                provide(|Inject(ca): Inject<CA>| Ok(C(ca))),
            ],
            scope(Step) [
                provide(|Inject(b): Inject<B>, Inject(c): Inject<C>| Ok(A(b, c))),
            ],
        });
        let request_container = app_container.clone().enter_build().unwrap();
        let action_container = request_container.clone().enter_build().unwrap();
        let step_container = action_container.clone().enter_build().unwrap();

        let _ = step_container.get::<A>().unwrap();
        let _ = step_container.get::<CAAAAA>().unwrap();
        let _ = step_container.get::<CAAAA>().unwrap();
        let _ = step_container.get::<CAAA>().unwrap();
        let _ = step_container.get::<CAA>().unwrap();
        let _ = step_container.get::<CA>().unwrap();
        let _ = step_container.get::<C>().unwrap();
        let _ = step_container.get::<B>().unwrap();
    }

    struct RequestTransient1;
    struct RequestTransient2(RequestTransient1);
    struct RequestTransient3(RequestTransient1, RequestTransient2);

    #[test]
    #[traced_test]
    fn test_transient_get() {
        let app_container = Container::new(registry! {
            scope(App) [
                provide(|| Ok(RequestTransient1)),
            ],
            scope(Request) [
                provide(|InjectTransient(req): InjectTransient<RequestTransient1>| Ok(RequestTransient2(req))),
                provide(|InjectTransient(req_1): InjectTransient<RequestTransient1>, InjectTransient(req_2): InjectTransient<RequestTransient2>| {
                    Ok(RequestTransient3(req_1, req_2))
                }),
            ]
        });
        let request_container = app_container.clone().enter().with_scope(Request).build().unwrap();

        assert!(app_container.get_transient::<RequestTransient1>().is_ok());
        assert!(matches!(
            app_container.get_transient::<RequestTransient2>(),
            Err(ResolveErrorKind::NoAccessible {
                expected_scope_data: _,
                actual_scope_data: _,
            }),
        ));
        assert!(matches!(
            app_container.get_transient::<RequestTransient3>(),
            Err(ResolveErrorKind::NoAccessible {
                expected_scope_data: _,
                actual_scope_data: _,
            }),
        ));

        assert!(request_container.get_transient::<RequestTransient1>().is_ok());
        assert!(request_container.get_transient::<RequestTransient2>().is_ok());
        assert!(request_container.get_transient::<RequestTransient3>().is_ok());
    }

    #[test]
    #[traced_test]
    fn test_scope_hierarchy() {
        let app_container = Container::new(registry! {
            scope(Runtime) [
                provide(|| Ok(())),
            ],
            scope(App) [
                provide(|| Ok(((), ()))),
            ],
            scope(Session) [
                provide(|| Ok(((), (), ()))),
            ],
            scope(Request) [
                provide(|| Ok(((), (), (), ()))),
            ],
            scope(Action) [
                provide(|| Ok(((), (), (), (), ()))),
            ],
            scope(Step) [
                provide(|| Ok(((), (), (), (), (), ()))),
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
        assert_eq!(
            app_container_inner.parent.as_ref().unwrap().inner.scope_data.priority,
            Runtime.priority()
        );
        assert_eq!(app_container_inner.child_scopes_data.len(), 4);
        assert_eq!(app_container_inner.scope_data.priority, App.priority());

        // Session scope is skipped by default, but it is still present in the child registries
        assert_eq!(
            request_container_inner.parent.as_ref().unwrap().inner.scope_data.priority,
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

    #[test]
    #[traced_test]
    fn test_scope_with_hierarchy() {
        let runtime_container = Container::new_with_start_scope(
            registry! {
                scope(Runtime) [
                    provide(|| Ok(())),
                ],
                scope(App) [
                    provide(|| Ok(((), ()))),
                ],
                scope(Session) [
                    provide(|| Ok(((), (), ()))),
                ],
                scope(Request) [
                    provide(|| Ok(((), (), (), ()))),
                ],
                scope(Action) [
                    provide(|| Ok(((), (), (), (), ()))),
                ],
                scope(Step) [
                    provide(|| Ok(((), (), (), (), (), ()))),
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

    #[test]
    #[traced_test]
    fn test_close_for_unresolved() {
        let finalizer_1_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_2_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_3_request_call_count = RcThreadSafety::new(AtomicU8::new(0));

        let app_container = Container::new(registry! {
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
                        let finalizer_3_request_call_count = finalizer_3_request_call_count.clone();
                        move |_: RcThreadSafety<((), (), (), ())>| {
                            finalizer_3_request_call_count.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                ),
            ],
        });
        let request_container = app_container.clone().enter().with_scope(Request).build().unwrap();

        request_container.close();
        app_container.close();

        assert_eq!(finalizer_1_request_call_count.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_2_request_call_count.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_3_request_call_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    #[traced_test]
    fn test_close_for_resolved() {
        let request_call_count = RcThreadSafety::new(AtomicU8::new(0));

        let finalizer_1_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_1_request_call_position = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_2_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_2_request_call_position = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_3_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_3_request_call_position = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_4_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_4_request_call_position = RcThreadSafety::new(AtomicU8::new(0));

        let app_container = Container::new(registry! {
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
                        let finalizer_3_request_call_position = finalizer_3_request_call_position.clone();
                        let finalizer_3_request_call_count = finalizer_3_request_call_count.clone();
                        move |_: RcThreadSafety<((), (), (), ())>| {
                            request_call_count.fetch_add(1, Ordering::SeqCst);
                            finalizer_3_request_call_position.store(request_call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                            finalizer_3_request_call_count.fetch_add(1, Ordering::SeqCst);

                            debug!("Finalizer 3 called");
                        }
                    }
                ),
                provide(
                    || Ok(((), (), (), (), ())),
                    finalizer = {
                        let request_call_count = request_call_count.clone();
                        let finalizer_4_request_call_position = finalizer_4_request_call_position.clone();
                        let finalizer_4_request_call_count = finalizer_4_request_call_count.clone();
                        move |_: RcThreadSafety<((), (), (), (), ())>| {
                            request_call_count.fetch_add(1, Ordering::SeqCst);
                            finalizer_4_request_call_position.store(request_call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                            finalizer_4_request_call_count.fetch_add(1, Ordering::SeqCst);

                            debug!("Finalizer 4 called");
                        }
                    }
                ),
            ],
        });
        let request_container = app_container.clone().enter().with_scope(Request).build().unwrap();

        let _ = request_container.get::<()>().unwrap();
        let _ = request_container.get::<((), ())>().unwrap();
        let _ = request_container.get::<((), (), (), (), ())>().unwrap();
        let _ = request_container.get::<((), (), (), ())>().unwrap();

        let runtime_container_resolved_set_count = app_container.inner.parent.as_ref().unwrap().inner.cache.lock().resolved.0.len();
        let app_container_resolved_set_count = app_container.inner.cache.lock().resolved.0.len();
        let request_container_resolved_set_count = request_container.inner.cache.lock().resolved.0.len();

        request_container.close();

        assert_eq!(runtime_container_resolved_set_count, 1);
        assert_eq!(app_container_resolved_set_count, 1);
        assert_eq!(request_container_resolved_set_count, 2);

        assert_eq!(finalizer_1_request_call_count.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_1_request_call_position.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_2_request_call_count.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_2_request_call_position.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_3_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_3_request_call_position.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_4_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_4_request_call_position.load(Ordering::SeqCst), 2);

        app_container.close();

        assert_eq!(finalizer_1_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_1_request_call_position.load(Ordering::SeqCst), 4);
        assert_eq!(finalizer_2_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_2_request_call_position.load(Ordering::SeqCst), 3);
        assert_eq!(finalizer_3_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_3_request_call_position.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_4_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_4_request_call_position.load(Ordering::SeqCst), 2);
    }

    #[test]
    #[traced_test]
    fn test_close_on_drop() {
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

        let app_container = Container::new(registry! {
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
    #[traced_test]
    fn test_thread_safe() {
        struct Request1 {
            #[cfg(not(feature = "thread_safe"))]
            _phantom: core::marker::PhantomData<*const ()>,
        }

        fn impl_bounds<T: Send + Sync + 'static>() {}

        impl_bounds::<(Container, ContainerInner)>();

        let app_container = Container::new(registry! {
            scope(App) [
                provide(|| Ok(RequestTransient1)),
            ],
        });
        std::thread::spawn(move || {
            let request1 = app_container.get_transient::<Request1>();
            let request2 = app_container.get::<Request1>();

            assert!(request1.is_ok());
            assert!(request2.is_ok());
        });
    }
}
