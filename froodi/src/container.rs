use core::any::{type_name, TypeId};

use alloc::boxed::Box;
use parking_lot::Mutex;
use tracing::{debug, debug_span, error, warn};

use super::{cache::Cache, registry::RegistryBuilder};
use crate::{
    cache::Resolved,
    context::Context,
    errors::{InstantiatorErrorKind, ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind},
    registry::{InstantiatorInnerData, ScopedRegistry},
    scope::Scope,
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
    pub fn new<S: Scope>(registry_builder: RegistryBuilder<S>) -> Self {
        let mut registries = registry_builder.build().into_iter();
        let (root_registry, child_registries) = if let Some(root_registry) = registries.next() {
            (RcThreadSafety::new(root_registry), registries.map(RcThreadSafety::new).collect())
        } else {
            panic!("registries len (is 0) should be > 1");
        };

        let mut container = BoxedContainerInner {
            cache: Cache::new(),
            context: Context::new(),
            root_registry,
            child_registries,
            parent: None,
            close_parent: false,
        };

        let mut iter = container.child_registries.iter();
        let mut registry = (*iter.next().expect("registries len (is 1) should be > 1")).clone();
        let mut child_registries = iter.cloned().collect();

        let mut search_next = container.root_registry.scope.is_skipped_by_default;
        while search_next {
            container = container.init_child(registry, child_registries, true);

            search_next = container.root_registry.scope.is_skipped_by_default;
            if search_next {
                let mut iter = container.child_registries.iter();
                registry = (*iter.next().expect("last scope can't be skipped by default")).clone();
                child_registries = iter.cloned().collect();
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
    pub fn new_with_start_scope<S: Scope>(registry_builder: RegistryBuilder<S>, scope: S) -> Self {
        let mut registries = registry_builder.build().into_iter();
        let (root_registry, child_registries) = if let Some(root_registry) = registries.next() {
            (RcThreadSafety::new(root_registry), registries.map(RcThreadSafety::new).collect())
        } else {
            panic!("registries len (is 0) should be > 1");
        };

        let container_priority = root_registry.scope.priority;
        let priority = scope.priority();

        let mut container = BoxedContainerInner {
            cache: Cache::new(),
            context: Context::new(),
            root_registry,
            child_registries,
            parent: None,
            close_parent: false,
        };

        if container_priority == priority {
            return container.into();
        }

        let mut iter = container.child_registries.iter();
        let mut registry = (*iter.next().expect("last scope can't be with another priority")).clone();
        let mut child_registries = iter.cloned().collect();

        let mut search_next = container.root_registry.scope.priority != priority;
        while search_next {
            container = container.init_child(registry, child_registries, true);

            search_next = container.root_registry.scope.priority != priority;
            if search_next {
                let mut iter = container.child_registries.iter();
                registry = (*iter.next().expect("last scope can't be with another priority")).clone();
                child_registries = iter.cloned().collect();
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
    #[allow(clippy::missing_errors_doc)]
    pub fn get<Dep: SendSafety + SyncSafety + 'static>(&self) -> Result<RcThreadSafety<Dep>, ResolveErrorKind> {
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
                return match parent.get::<Dep>() {
                    Ok(dependency) => {
                        self.inner.cache.lock().insert_rc(dependency.clone());
                        Ok(dependency)
                    }
                    Err(err) => Err(err),
                };
            }

            let err = ResolveErrorKind::NoInstantiator;
            warn!("{}", err);
            return Err(err);
        };

        match instantiator.call(self.clone()) {
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
    #[allow(clippy::missing_errors_doc)]
    pub fn get_transient<Dep: 'static>(&self) -> Result<Dep, ResolveErrorKind> {
        let span = debug_span!("resolve", dependency = type_name::<Dep>());
        let _guard = span.enter();

        let type_id = TypeId::of::<Dep>();

        let Some(mut instantiator) = self.inner.root_registry.get_instantiator(&type_id) else {
            if let Some(parent) = &self.inner.parent {
                debug!("No instantiator found, trying parent container");
                return parent.get_transient();
            }

            let err = ResolveErrorKind::NoInstantiator;
            warn!("{}", err);
            return Err(err);
        };

        match instantiator.call(self.clone()) {
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
        root_registry: RcThreadSafety<ScopedRegistry>,
        child_registries: Box<[RcThreadSafety<ScopedRegistry>]>,
        close_parent: bool,
    ) -> Container {
        let mut cache = self.inner.cache.lock().child();
        cache.append_context(&mut context.clone());

        Container {
            inner: RcThreadSafety::new(ContainerInner {
                cache: Mutex::new(cache),
                context: Mutex::new(context),
                root_registry,
                child_registries,
                parent: Some(self),
                close_parent,
            }),
        }
    }

    #[inline]
    #[must_use]
    fn init_child(
        self,
        root_registry: RcThreadSafety<ScopedRegistry>,
        child_registries: Box<[RcThreadSafety<ScopedRegistry>]>,
        close_parent: bool,
    ) -> Container {
        let mut cache = self.inner.cache.lock().child();
        let context = self.inner.context.lock().clone();
        cache.append_context(&mut context.clone());

        Container {
            inner: RcThreadSafety::new(ContainerInner {
                cache: Mutex::new(cache),
                context: Mutex::new(context),
                root_registry,
                child_registries,
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

        let mut iter = self.container.inner.child_registries.iter();
        let registry = (*iter.next().ok_or(NoChildRegistries)?).clone();
        let child_registries = iter.cloned().collect();

        let mut child = self.container.init_child(registry, child_registries, false);
        while child.inner.root_registry.scope.is_skipped_by_default {
            let mut iter = child.inner.child_registries.iter();
            let registry = (*iter.next().ok_or(NoNonSkippedRegistries)?).clone();
            let child_registries = iter.cloned().collect();

            child = child.init_child(registry, child_registries, true);
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

        let mut child = self.container.init_child(registry, child_registries, false);
        while child.inner.root_registry.scope.priority != priority {
            let mut iter = child.inner.child_registries.iter();
            let registry = (*iter.next().ok_or(NoChildRegistriesWithScope {
                name: self.scope.name(),
                priority,
            })?)
            .clone();
            let child_registries = iter.cloned().collect();

            child = child.init_child(registry, child_registries, true);
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

        let mut child = self
            .container
            .init_child_with_context(self.context.clone(), registry, child_registries, false);
        while child.inner.root_registry.scope.is_skipped_by_default {
            let mut iter = child.inner.child_registries.iter();
            let registry = (*iter.next().ok_or(NoNonSkippedRegistries)?).clone();
            let child_registries = iter.cloned().collect();

            child = child.init_child_with_context(self.context.clone(), registry, child_registries, true);
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

        let mut child = self
            .container
            .init_child_with_context(self.context.clone(), registry, child_registries, false);
        while child.inner.root_registry.scope.priority != priority {
            let mut iter = child.inner.child_registries.iter();
            let registry = (*iter.next().ok_or(NoChildRegistriesWithScope {
                name: self.scope.name(),
                priority,
            })?)
            .clone();
            let child_registries = iter.cloned().collect();

            child = child.init_child_with_context(self.context.clone(), registry, child_registries, true);
        }

        Ok(child)
    }
}

#[derive(Clone)]
pub(crate) struct BoxedContainerInner {
    pub(crate) cache: Cache,
    pub(crate) context: Context,
    pub(crate) root_registry: RcThreadSafety<ScopedRegistry>,
    pub(crate) child_registries: Box<[RcThreadSafety<ScopedRegistry>]>,
    pub(crate) parent: Option<Box<BoxedContainerInner>>,
    pub(crate) close_parent: bool,
}

impl BoxedContainerInner {
    #[inline]
    #[must_use]
    pub(crate) fn init_child(
        self,
        root_registry: RcThreadSafety<ScopedRegistry>,
        child_registries: Box<[RcThreadSafety<ScopedRegistry>]>,
        close_parent: bool,
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
        }
    }
}

pub(crate) struct ContainerInner {
    pub(crate) cache: Mutex<Cache>,
    pub(crate) context: Mutex<Context>,
    pub(crate) root_registry: RcThreadSafety<ScopedRegistry>,
    pub(crate) child_registries: Box<[RcThreadSafety<ScopedRegistry>]>,
    pub(crate) parent: Option<Container>,
    pub(crate) close_parent: bool,
}

impl ContainerInner {
    #[allow(clippy::missing_panics_doc)]
    fn close(&self) {
        self.close_with_parent_flag(self.close_parent);
    }

    pub(crate) fn close_with_parent_flag(&self, close_parent: bool) {
        let mut resolved_set = self.cache.lock().take_resolved_set();
        while let Some(Resolved { type_id, dependency }) = resolved_set.0.pop_back() {
            let InstantiatorInnerData { finalizer, .. } = self
                .root_registry
                .get_instantiator_data(&type_id)
                .expect("Instantiator should be present for resolved type");

            if let Some(mut finalizer) = finalizer {
                let _ = finalizer.call(dependency);
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

    use super::{Container, RegistryBuilder};
    use crate::{
        container::ContainerInner,
        inject::{Inject, InjectTransient},
        scope::DefaultScope::*,
        utils::thread_safety::RcThreadSafety,
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

        assert!(app_container.get_transient::<RequestTransient1>().is_ok());
        assert!(app_container.get_transient::<RequestTransient2>().is_err());
        assert!(app_container.get_transient::<RequestTransient3>().is_err());

        assert!(request_container.get_transient::<RequestTransient1>().is_ok());
        assert!(request_container.get_transient::<RequestTransient2>().is_ok());
        assert!(request_container.get_transient::<RequestTransient3>().is_ok());
    }

    #[test]
    #[traced_test]
    fn test_scope_hierarchy() {
        let registry = RegistryBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(((), ())), App)
            .provide(|| Ok(((), (), ())), Session)
            .provide(|| Ok(((), (), (), ())), Request)
            .provide(|| Ok(((), (), (), (), ())), Action)
            .provide(|| Ok(((), (), (), (), (), ())), Step);

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

    #[test]
    #[traced_test]
    fn test_scope_with_hierarchy() {
        let registry = RegistryBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(((), ())), App)
            .provide(|| Ok(((), (), ())), Session)
            .provide(|| Ok(((), (), (), ())), Request)
            .provide(|| Ok(((), (), (), (), ())), Action)
            .provide(|| Ok(((), (), (), (), (), ())), Step);

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

    #[test]
    #[traced_test]
    fn test_close_for_unresolved() {
        let finalizer_1_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_2_request_call_count = RcThreadSafety::new(AtomicU8::new(0));
        let finalizer_3_request_call_count = RcThreadSafety::new(AtomicU8::new(0));

        let registry = RegistryBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(((), ())), App)
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
            .add_finalizer({
                let finalizer_3_request_call_count = finalizer_3_request_call_count.clone();
                move |_: RcThreadSafety<((), (), (), ())>| {
                    finalizer_3_request_call_count.fetch_add(1, Ordering::SeqCst);
                }
            });

        let app_container = Container::new(registry);
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

        let registry = RegistryBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(((), ())), App)
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
            .add_finalizer({
                let request_call_count = request_call_count.clone();
                let finalizer_3_request_call_position = finalizer_3_request_call_position.clone();
                let finalizer_3_request_call_count = finalizer_3_request_call_count.clone();
                move |_: RcThreadSafety<((), (), (), ())>| {
                    request_call_count.fetch_add(1, Ordering::SeqCst);
                    finalizer_3_request_call_position.store(request_call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                    finalizer_3_request_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Finalizer 3 called");
                }
            })
            .add_finalizer({
                let request_call_count = request_call_count.clone();
                let finalizer_4_request_call_position = finalizer_4_request_call_position.clone();
                let finalizer_4_request_call_count = finalizer_4_request_call_count.clone();
                move |_: RcThreadSafety<((), (), (), (), ())>| {
                    request_call_count.fetch_add(1, Ordering::SeqCst);
                    finalizer_4_request_call_position.store(request_call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                    finalizer_4_request_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Finalizer 4 called");
                }
            });

        let app_container = Container::new(registry);
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

        let registry = RegistryBuilder::new().provide(|| Ok(RequestTransient1), App);
        let app_container = Container::new(registry);
        std::thread::spawn(move || {
            let request1 = app_container.get_transient::<Request1>();
            let request2 = app_container.get::<Request1>();

            assert!(request1.is_ok());
            assert!(request2.is_ok());
        });
    }
}
