use core::any::{type_name, TypeId};

use alloc::{boxed::Box, sync::Arc};
use parking_lot::Mutex;
use tracing::{debug, debug_span, error, warn};

use super::{cache::Cache, registry::RegistriesBuilder};
use crate::{
    cache::Resolved,
    context::Context,
    errors::{ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind},
    registry::{InstantiatorInnerData, Registry},
    scope::Scope,
    service::Service as _,
    InstantiatorErrorKind,
};

#[derive(Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
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
    pub fn close(&mut self) {
        while let Some(Resolved { type_id, dependency }) = self.cache.get_resolved_set_mut().0.pop_back() {
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
            self.cache.map = self.context.map.clone();
        }

        if self.close_parent {
            if let Some(parent) = &self.parent {
                parent.close();
                debug!("Parent container closed");
            }
        }
    }
}

#[cfg(feature = "eq")]
impl PartialEq for ContainerInner {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.root_registry, &other.root_registry)
            && self.cache == other.cache
            && self.context == other.context
            && self.child_registries.len() == other.child_registries.len()
            && self
                .child_registries
                .iter()
                .zip(other.child_registries.iter())
                .all(|(a, b)| Arc::ptr_eq(a, b))
            && self.parent == other.parent
    }
}

#[cfg(feature = "eq")]
impl Eq for ContainerInner {}

impl Drop for ContainerInner {
    fn drop(&mut self) {
        self.close();
        debug!("Container closed on drop");
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Container {
    inner: Arc<Mutex<ContainerInner>>,
}

impl Container {
    /// # Panics
    /// Panics if registries builder doesn't create any registry.
    /// This can occur if scopes are empty.
    #[inline]
    #[must_use]
    #[allow(clippy::new_ret_no_self)]
    pub fn new<S: Scope>(registries_builder: RegistriesBuilder<S>) -> Container {
        let mut registries = registries_builder.build().into_iter();
        let (root_registry, child_registries) = if let Some(root_registry) = registries.next() {
            (Arc::new(root_registry), registries.map(Arc::new).collect())
        } else {
            panic!("registries len (is 0) should be >= 1");
        };

        Self {
            inner: Arc::new(Mutex::new(ContainerInner {
                cache: Cache::new(),
                context: Context::new(),
                root_registry,
                child_registries,
                parent: None,
                close_parent: false,
            })),
        }
    }

    /// Creates child container builder
    ///
    /// # Warning
    /// This method requires `self` instead of `&self` because it consumes the container,
    /// so be careful when want to clone container before calling this method,
    /// because these containers will be different and using different state like cache,
    /// so `close` will not work as expected for parent container that was cloned to create child container and used after.
    #[inline]
    #[must_use]
    pub fn enter(self) -> ChildContainerBuiler {
        ChildContainerBuiler { container: self }
    }

    /// Creates child container and builds it with next non-skipped scope
    ///
    /// # Warning
    /// - This method requires `self` instead of `&self` because it consumes the container,
    ///   so be careful when want to clone container before calling this method,
    ///   because these containers will be different and using different state like cache,
    ///   so `close` will not work as expected for parent container that was cloned to create child container and used after.
    /// - This method skips skipped scopes, if you want to use one of them, use [`ChildContainerBuiler::with_scope`]
    /// - If you want to use specific scope, use [`ChildContainerBuiler::with_scope`]
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
    pub fn get<Dep: Send + Sync + 'static>(&self) -> Result<Arc<Dep>, ResolveErrorKind> {
        let span = debug_span!("resolve", dependency = type_name::<Dep>());
        let _guard = span.enter();

        let type_id = TypeId::of::<Dep>();

        if let Some(dependency) = self.inner.lock().cache.get(&type_id) {
            debug!("Found in cache");
            return Ok(dependency);
        }
        debug!("Not found in cache");

        let guard = self.inner.lock();
        let Some(InstantiatorInnerData {
            mut instantiator,
            finalizer,
            config,
        }) = guard.root_registry.get_instantiator_data(&type_id)
        else {
            if let Some(parent) = &guard.parent {
                debug!("No instantiator found, trying parent container");
                return match parent.get::<Dep>() {
                    Ok(dependency) => {
                        drop(guard);
                        let mut guard = self.inner.lock();
                        guard.cache.insert_rc(dependency.clone());
                        Ok(dependency)
                    }
                    Err(err) => Err(err),
                };
            }
            drop(guard);

            let err = ResolveErrorKind::NoInstantiator;
            warn!("{}", err);
            return Err(err);
        };
        drop(guard);

        match instantiator.call(self.clone()) {
            Ok(dependency) => match dependency.downcast::<Dep>() {
                Ok(dependency) => {
                    let dependency = Arc::new(*dependency);
                    let mut guard = self.inner.lock();
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
    #[allow(clippy::missing_errors_doc)]
    pub fn get_transient<Dep: 'static>(&self) -> Result<Dep, ResolveErrorKind> {
        let span = debug_span!("resolve", dependency = type_name::<Dep>());
        let _guard = span.enter();

        let type_id = TypeId::of::<Dep>();

        let guard = self.inner.lock();
        let Some(mut instantiator) = guard.root_registry.get_instantiator(&type_id) else {
            if let Some(parent) = &guard.parent {
                debug!("No instantiator found, trying parent container");
                return parent.get_transient();
            }
            drop(guard);

            let err = ResolveErrorKind::NoInstantiator;
            warn!("{}", err);
            return Err(err);
        };
        drop(guard);

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
        self.inner.lock().close();
    }
}

impl Container {
    #[inline]
    #[must_use]
    fn init_child_with_context(
        self,
        context: Context,
        root_registry: Arc<Registry>,
        child_registries: Box<[Arc<Registry>]>,
        close_parent: bool,
    ) -> Container {
        let inner = self.inner.lock();

        let mut cache = inner.cache.child();
        cache.append_context(&context);

        drop(inner);

        Container {
            inner: Arc::new(Mutex::new(ContainerInner {
                cache,
                context,
                root_registry,
                child_registries,
                parent: Some(self),
                close_parent,
            })),
        }
    }

    #[inline]
    #[must_use]
    fn init_child(self, root_registry: Arc<Registry>, child_registries: Box<[Arc<Registry>]>, close_parent: bool) -> Container {
        let inner = self.inner.lock();

        let mut cache = inner.cache.child();
        let context = inner.context.clone();
        cache.append_context(&context);

        drop(inner);

        Container {
            inner: Arc::new(Mutex::new(ContainerInner {
                cache,
                context,
                root_registry,
                child_registries,
                parent: Some(self),
                close_parent,
            })),
        }
    }
}

#[cfg(feature = "eq")]
impl PartialEq for Container {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

#[cfg(feature = "eq")]
impl Eq for Container {}

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
    ///
    /// # Errors
    /// - Returns [`ScopeErrorKind::NoChildRegistries`] if there are no registries
    /// - Returns [`ScopeErrorKind::NoNonSkippedRegistries`] if there are no non-skipped registries
    ///
    /// # Warning
    /// - This method skips skipped scopes, if you want to use one of them, use [`ChildContainerBuiler::with_scope`]
    /// - If you want to use specific scope, use [`ChildContainerBuiler::with_scope`]
    pub fn build(self) -> Result<Container, ScopeErrorKind> {
        use ScopeErrorKind::{NoChildRegistries, NoNonSkippedRegistries};

        let inner = self.container.inner.lock();
        let mut iter = inner.child_registries.iter();
        let registry = (*iter.next().ok_or(NoChildRegistries)?).clone();
        let child_registries = iter.cloned().collect();
        drop(inner);

        let mut child = self.container.init_child(registry, child_registries, false);
        let mut inner = child.inner.lock();
        while inner.root_registry.scope.is_skipped_by_default {
            let mut iter = inner.child_registries.iter();
            let registry = (*iter.next().ok_or(NoNonSkippedRegistries)?).clone();
            let child_registries = iter.cloned().collect();

            drop(inner);
            child = child.init_child(registry, child_registries, true);
            inner = child.inner.lock();
        }
        drop(inner);

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

        let inner = self.container.inner.lock();
        let mut iter = inner.child_registries.iter();
        let registry = (*iter.next().ok_or(NoChildRegistries)?).clone();
        let child_registries = iter.cloned().collect();
        drop(inner);

        let mut child = self.container.init_child(registry, child_registries, false);
        let mut inner = child.inner.lock();
        while inner.root_registry.scope.priority != priority {
            let mut iter = inner.child_registries.iter();
            let registry = (*iter.next().ok_or(NoChildRegistriesWithScope {
                name: self.scope.name(),
                priority,
            })?)
            .clone();
            let child_registries = iter.cloned().collect();

            drop(inner);
            child = child.init_child(registry, child_registries, true);
            inner = child.inner.lock();
        }
        drop(inner);

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
    /// - This method skips skipped scopes, if you want to use one of them, use [`ChildContainerBuiler::with_scope`]
    /// - If you want to use specific scope, use [`ChildContainerBuiler::with_scope`]
    pub fn build(self) -> Result<Container, ScopeErrorKind> {
        use ScopeErrorKind::{NoChildRegistries, NoNonSkippedRegistries};

        let inner = self.container.inner.lock();
        let mut iter = inner.child_registries.iter();
        let registry = (*iter.next().ok_or(NoChildRegistries)?).clone();
        let child_registries = iter.cloned().collect();
        drop(inner);

        let mut child = self
            .container
            .init_child_with_context(self.context.clone(), registry, child_registries, false);
        let mut inner = child.inner.lock();
        while inner.root_registry.scope.is_skipped_by_default {
            let mut iter = inner.child_registries.iter();
            let registry = (*iter.next().ok_or(NoNonSkippedRegistries)?).clone();
            let child_registries = iter.cloned().collect();

            drop(inner);
            child = child.init_child_with_context(self.context.clone(), registry, child_registries, true);
            inner = child.inner.lock();
        }
        drop(inner);

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

        let inner = self.container.inner.lock();
        let mut iter = inner.child_registries.iter();
        let registry = (*iter.next().ok_or(NoChildRegistries)?).clone();
        let child_registries = iter.cloned().collect();
        drop(inner);

        let mut child = self
            .container
            .init_child_with_context(self.context.clone(), registry, child_registries, false);
        let mut inner = child.inner.lock();
        while inner.root_registry.scope.priority != priority {
            let mut iter = inner.child_registries.iter();
            let registry = (*iter.next().ok_or(NoChildRegistriesWithScope {
                name: self.scope.name(),
                priority,
            })?)
            .clone();
            let child_registries = iter.cloned().collect();

            drop(inner);
            child = child.init_child_with_context(self.context.clone(), registry, child_registries, true);
            inner = child.inner.lock();
        }
        drop(inner);

        Ok(child)
    }
}

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    extern crate std;

    use super::{Container, RegistriesBuilder};
    use crate::{container::ContainerInner, scope::DefaultScope::*, Inject, InjectTransient, Scope};

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

    #[test]
    #[traced_test]
    fn test_scoped_get() {
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
            .provide(|Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa)), App)
            .provide(|Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa)), Session)
            .provide(|Inject(caaa): Inject<CAAA>| Ok(CAA(caaa)), Request)
            .provide(|Inject(caa): Inject<CAA>| Ok(CA(caa)), Request)
            .provide(|Inject(ca): Inject<CA>| Ok(C(ca)), Action)
            .provide(|| Ok(B(2)), App)
            .provide(|Inject(b): Inject<B>, Inject(c): Inject<C>| Ok(A(b, c)), Step);
        let runtime_container = Container::new(registry);
        let app_container = runtime_container.clone().enter_build().unwrap();
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
        let registry = RegistriesBuilder::new()
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
        let registry = RegistriesBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(((), ())), App)
            .provide(|| Ok(((), (), ())), Session)
            .provide(|| Ok(((), (), (), ())), Request)
            .provide(|| Ok(((), (), (), (), ())), Action)
            .provide(|| Ok(((), (), (), (), (), ())), Step);

        let runtime_container = Container::new(registry);
        let app_container = runtime_container.clone().enter_build().unwrap();
        let request_container = app_container.clone().enter_build().unwrap();
        let action_container = request_container.clone().enter_build().unwrap();
        let step_container = action_container.clone().enter_build().unwrap();

        let runtime_container_inner = runtime_container.inner.lock();
        let app_container_inner = app_container.inner.lock();
        let request_container_inner = request_container.inner.lock();
        let action_container_inner = action_container.inner.lock();
        let step_container_inner = step_container.inner.lock();

        assert_eq!(runtime_container_inner.parent, None);
        assert_eq!(runtime_container_inner.child_registries.len(), 5);
        assert_eq!(runtime_container_inner.root_registry.scope.priority, Runtime.priority());
        assert!(Arc::ptr_eq(
            &app_container_inner.root_registry,
            &runtime_container_inner.child_registries[0]
        ));

        drop(runtime_container_inner);

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

    #[test]
    #[traced_test]
    fn test_scope_with_hierarchy() {
        let registry = RegistriesBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(((), ())), App)
            .provide(|| Ok(((), (), ())), Session)
            .provide(|| Ok(((), (), (), ())), Request)
            .provide(|| Ok(((), (), (), (), ())), Action)
            .provide(|| Ok(((), (), (), (), (), ())), Step);

        let runtime_container = Container::new(registry);
        let app_container = runtime_container.clone().enter().with_scope(App).build().unwrap();
        let session_container = runtime_container.clone().enter().with_scope(Session).build().unwrap();
        let request_container = app_container.clone().enter().with_scope(Request).build().unwrap();
        let action_container = request_container.clone().enter().with_scope(Action).build().unwrap();
        let step_container = action_container.clone().enter().with_scope(Step).build().unwrap();

        let runtime_container_inner = runtime_container.inner.lock();
        let app_container_inner = app_container.inner.lock();
        let session_container_inner = session_container.inner.lock();
        let request_container_inner = request_container.inner.lock();
        let action_container_inner = action_container.inner.lock();
        let step_container_inner = step_container.inner.lock();

        assert_eq!(runtime_container_inner.parent, None);
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

    #[test]
    #[traced_test]
    fn test_close_for_unresolved() {
        let finalizer_1_request_call_count = Arc::new(AtomicU8::new(0));
        let finalizer_2_request_call_count = Arc::new(AtomicU8::new(0));
        let finalizer_3_request_call_count = Arc::new(AtomicU8::new(0));

        let registry = RegistriesBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(((), ())), App)
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
            .add_finalizer({
                let finalizer_3_request_call_count = finalizer_3_request_call_count.clone();
                move |_: Arc<((), (), (), ())>| {
                    finalizer_3_request_call_count.fetch_add(1, Ordering::SeqCst);
                }
            });

        let runtime_container = Container::new(registry);
        let app_container = runtime_container.clone().enter().with_scope(App).build().unwrap();
        let request_container = app_container.clone().enter().with_scope(Request).build().unwrap();

        request_container.close();
        app_container.close();
        runtime_container.close();

        assert_eq!(finalizer_1_request_call_count.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_2_request_call_count.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_3_request_call_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    #[traced_test]
    fn test_close_for_resolved() {
        let request_call_count = Arc::new(AtomicU8::new(0));

        let finalizer_1_request_call_count = Arc::new(AtomicU8::new(0));
        let finalizer_1_request_call_position = Arc::new(AtomicU8::new(0));
        let finalizer_2_request_call_count = Arc::new(AtomicU8::new(0));
        let finalizer_2_request_call_position = Arc::new(AtomicU8::new(0));
        let finalizer_3_request_call_count = Arc::new(AtomicU8::new(0));
        let finalizer_3_request_call_position = Arc::new(AtomicU8::new(0));
        let finalizer_4_request_call_count = Arc::new(AtomicU8::new(0));
        let finalizer_4_request_call_position = Arc::new(AtomicU8::new(0));

        let registry = RegistriesBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(((), ())), App)
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
            .add_finalizer({
                let request_call_count = request_call_count.clone();
                let finalizer_3_request_call_position = finalizer_3_request_call_position.clone();
                let finalizer_3_request_call_count = finalizer_3_request_call_count.clone();
                move |_: Arc<((), (), (), ())>| {
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
                move |_: Arc<((), (), (), (), ())>| {
                    request_call_count.fetch_add(1, Ordering::SeqCst);
                    finalizer_4_request_call_position.store(request_call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                    finalizer_4_request_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Finalizer 4 called");
                }
            });

        let runtime_container = Container::new(registry);
        let app_container = runtime_container.clone().enter().with_scope(App).build().unwrap();
        let request_container = app_container.clone().enter().with_scope(Request).build().unwrap();

        let _ = request_container.get::<()>().unwrap();
        let _ = request_container.get::<((), ())>().unwrap();
        let _ = request_container.get::<((), (), (), (), ())>().unwrap();
        let _ = request_container.get::<((), (), (), ())>().unwrap();

        let runtime_container_resolved_set_count = request_container
            .inner
            .lock()
            .parent
            .as_ref()
            .unwrap()
            .inner
            .lock()
            .parent
            .as_ref()
            .unwrap()
            .inner
            .lock()
            .cache
            .get_resolved_set()
            .0
            .len();
        let app_container_resolved_set_count = request_container
            .inner
            .lock()
            .parent
            .as_ref()
            .unwrap()
            .inner
            .lock()
            .cache
            .get_resolved_set()
            .0
            .len();
        let request_container_resolved_set_count = request_container.inner.lock().cache.get_resolved_set().0.len();

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

        assert_eq!(finalizer_1_request_call_count.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_1_request_call_position.load(Ordering::SeqCst), 0);
        assert_eq!(finalizer_2_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_2_request_call_position.load(Ordering::SeqCst), 3);
        assert_eq!(finalizer_3_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_3_request_call_position.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_4_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_4_request_call_position.load(Ordering::SeqCst), 2);

        runtime_container.close();

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
    fn test_bounds() {
        fn impl_bounds<T: Send + Sync + 'static>() {}

        impl_bounds::<(Container, ContainerInner)>();
    }
}
