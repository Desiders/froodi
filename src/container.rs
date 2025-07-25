use alloc::{boxed::Box, sync::Arc};
use parking_lot::Mutex;
use tracing::debug;

use super::{context::Context, dependency_resolver::DependencyResolver, registry::RegistriesBuilder};
use crate::{
    dependency_resolver::{Inject, InjectTransient, Resolved},
    errors::{ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind},
    registry::{InstantiatorInnerData, Registry},
    scope::Scope,
    service::Service as _,
};

#[derive(Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Container {
    context: Context,
    root_registry: Arc<Registry>,
    child_registries: Box<[Arc<Registry>]>,
    parent: Option<Box<Container>>,
    close_parent: bool,
}

#[cfg(feature = "eq")]
impl PartialEq for Container {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.root_registry, &other.root_registry)
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
impl Eq for Container {}

impl Container {
    /// # Panics
    /// Panics if registries builder doesn't create any registry.
    /// This can occur if scopes are empty.
    #[inline]
    #[must_use]
    pub fn new<S: Scope>(registries_builder: RegistriesBuilder<S>) -> Self {
        let mut registries = registries_builder.build().into_iter();
        let (root_registry, child_registries) = if let Some(root_registry) = registries.next() {
            (Arc::new(root_registry), registries.map(Arc::new).collect())
        } else {
            panic!("registries len (is 0) should be >= 1");
        };

        Self {
            context: Context::new(),
            root_registry,
            child_registries,
            parent: None,
            close_parent: false,
        }
    }

    #[inline]
    pub fn set_close_parent(&mut self, close_parent: bool) {
        self.close_parent = close_parent;
    }

    /// Creates child container builder
    ///
    /// # Warning
    /// This method requires `self` instead of `&self` because it consumes the container,
    /// so be careful when want to clone container before calling this method,
    /// because these containers will be different and using different state like context,
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
    ///   because these containers will be different and using different state like context,
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
    pub fn get<Dep: Send + Sync + 'static>(&mut self) -> Result<Arc<Dep>, ResolveErrorKind> {
        match Inject::resolve(self.root_registry.clone(), self.context.clone()) {
            Ok((Inject(dep), context)) => {
                self.context = context;
                Ok(dep)
            }
            Err(err @ ResolveErrorKind::NoInstantiator) => match &mut self.parent {
                Some(parent) => {
                    debug!("No instantiator found, trying parent container");
                    parent.get()
                }
                None => Err(err),
            },
            Err(err) => Err(err),
        }
    }

    /// Gets a transient dependency from the container
    ///
    /// # Notes
    /// This method resolves a new instance of the dependency each time it is called,
    /// so it should be used for dependencies that are not cached or shared, and without finalizer.
    #[allow(clippy::missing_errors_doc)]
    pub fn get_transient<Dep: 'static>(&mut self) -> Result<Dep, ResolveErrorKind> {
        match InjectTransient::resolve(self.root_registry.clone(), self.context.clone()) {
            Ok((InjectTransient(dep), context)) => {
                self.context = context;
                Ok(dep)
            }
            Err(err @ ResolveErrorKind::NoInstantiator) => match &mut self.parent {
                Some(parent) => {
                    debug!("No instantiator found, trying parent container");
                    parent.get_transient()
                }
                None => Err(err),
            },
            Err(err) => Err(err),
        }
    }

    /// Closes the container, calling finalizers for resolved dependencies in LIFO order.
    ///
    /// # Warning
    /// - This method can be called multiple times, but it will only call finalizers for dependencies that were resolved since the last call
    ///
    /// - If the container has a parent, it will also close the parent container if [`Self::close_parent`] is set to `true`
    #[allow(clippy::missing_panics_doc)]
    pub fn close(&mut self) {
        while let Some(Resolved { type_id, dependency }) = self.context.get_resolved_set_mut().0.pop_back() {
            let InstantiatorInnerData { finalizer, .. } = self
                .root_registry
                .get_instantiator_data(&type_id)
                .expect("Instantiator should be present for resolved type");

            if let Some(mut finalizer) = finalizer {
                let _ = finalizer.call(dependency);
                debug!(?type_id, "Finalizer called");
            }
        }

        if self.close_parent {
            if let Some(parent) = &mut self.parent {
                parent.close();
                debug!("Parent container closed");
            }
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

        let mut iter = self.container.child_registries.iter();
        let registry = iter.next().ok_or(NoChildRegistries)?;

        let mut child = self.container.init_child(registry.clone(), iter.cloned().collect());
        while child.root_registry.scope.is_skipped_by_default {
            let mut iter = child.child_registries.iter();
            let registry = iter.next().ok_or(NoNonSkippedRegistries)?;

            child = child.init_child(registry.clone(), iter.cloned().collect());
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

        let mut iter = self.container.child_registries.iter();
        let registry = iter.next().ok_or(NoChildRegistries)?;

        let mut child = self.container.init_child(registry.clone(), iter.cloned().collect());
        while child.root_registry.scope.priority != priority {
            let mut iter = child.child_registries.iter();
            let registry = iter.next().ok_or(NoChildRegistriesWithScope {
                name: self.scope.name(),
                priority,
            })?;

            child = child.init_child(registry.clone(), iter.cloned().collect());
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
    /// - This method skips skipped scopes, if you want to use one of them, use [`ChildContainerBuiler::with_scope`]
    /// - If you want to use specific scope, use [`ChildContainerBuiler::with_scope`]
    pub fn build(self) -> Result<Container, ScopeErrorKind> {
        use ScopeErrorKind::{NoChildRegistries, NoNonSkippedRegistries};

        let mut iter = self.container.child_registries.iter();
        let registry = iter.next().ok_or(NoChildRegistries)?;

        let mut child = self.container.init_child(registry.clone(), iter.cloned().collect());
        while child.root_registry.scope.is_skipped_by_default {
            let mut iter = child.child_registries.iter();
            let registry = iter.next().ok_or(NoNonSkippedRegistries)?;

            child = child.init_child_with_context(self.context.clone(), registry.clone(), iter.cloned().collect());
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

        let mut iter = self.container.child_registries.iter();
        let registry = iter.next().ok_or(NoChildRegistries)?;

        let mut child = self.container.init_child(registry.clone(), iter.cloned().collect());
        while child.root_registry.scope.priority != priority {
            let mut iter = child.child_registries.iter();
            let registry = iter.next().ok_or(NoChildRegistriesWithScope {
                name: self.scope.name(),
                priority,
            })?;

            child = child.init_child_with_context(self.context.clone(), registry.clone(), iter.cloned().collect());
        }

        Ok(child)
    }
}

impl Container {
    #[inline]
    #[must_use]
    fn init_child_with_context(&self, context: Context, root_registry: Arc<Registry>, child_registries: Box<[Arc<Registry>]>) -> Container {
        Container {
            context,
            root_registry,
            child_registries,
            parent: Some(Box::new(self.clone())),
            close_parent: true,
        }
    }

    #[inline]
    #[must_use]
    fn init_child(&self, root_registry: Arc<Registry>, child_registries: Box<[Arc<Registry>]>) -> Container {
        self.init_child_with_context(self.context.child(), root_registry, child_registries)
    }
}

impl Container {
    /// Creates a container that can be used in concurrent scenarios
    #[inline]
    #[must_use]
    pub fn shared(self) -> ContainerHandle {
        ContainerHandle {
            inner: Arc::new(Mutex::new(self)),
        }
    }
}

#[cfg(feature = "handle")]
mod handle {
    use alloc::sync::Arc;
    use parking_lot::Mutex;

    use crate::{container::ChildContainerBuiler, Container, ResolveErrorKind, ScopeErrorKind};

    #[derive(Clone)]
    pub struct ContainerHandle {
        pub(crate) inner: Arc<Mutex<Container>>,
    }

    impl ContainerHandle {
        /// Creates child container builder
        ///
        /// # Warning
        /// - The container is cloned before creating a child container,
        ///   so the child container will have its own state like context,
        ///   so `close` will not work as expected for the container that was cloned to create child container and used after.
        /// - `self` instead of `&self` is used to warn about this behavior
        #[inline]
        #[must_use]
        pub fn enter(self) -> ChildContainerBuiler {
            self.inner.lock().clone().enter()
        }

        /// Creates child container and builds it with next non-skipped scope
        ///
        /// # Warning
        /// - The container is cloned before creating a child container,
        ///   so the child container will have its own state like context,
        ///   so `close` will not work as expected for the container that was cloned to create child container and used after.
        /// - `self` instead of `&self` is used to warn about this behavior
        /// - This method skips skipped scopes, if you want to use one of them, use [`ChildContainerBuiler::with_scope`]
        /// - If you want to use specific scope, use [`ChildContainerBuiler::with_scope`]
        ///
        /// /// # Errors
        /// - Returns [`ScopeErrorKind::NoChildRegistries`] if there are no registries
        /// - Returns [`ScopeErrorKind::NoNonSkippedRegistries`] if there are no non-skipped registries
        #[inline]
        #[allow(clippy::missing_errors_doc)]
        pub fn enter_build(self) -> Result<Container, ScopeErrorKind> {
            self.inner.lock().clone().enter_build()
        }

        /// Gets a scoped dependency from the container
        ///
        /// # Notes
        /// This method resolves a dependency from the container,
        /// so it should be used for dependencies that are cached or shared,
        /// and with optional finalizer.
        #[inline]
        #[allow(clippy::missing_errors_doc)]
        pub fn get<Dep: Send + Sync + 'static>(&self) -> Result<Arc<Dep>, ResolveErrorKind> {
            self.inner.lock().get()
        }

        /// Gets a transient dependency from the container
        ///
        /// # Notes
        /// This method resolves a new instance of the dependency each time it is called,
        /// so it should be used for dependencies that are not cached or shared, and without finalizer.
        #[inline]
        #[allow(clippy::missing_errors_doc)]
        pub fn get_transient<Dep: 'static>(&self) -> Result<Dep, ResolveErrorKind> {
            self.inner.lock().get_transient()
        }

        /// Closes the container, calling finalizers for resolved dependencies in LIFO order.
        ///
        /// # Warning
        /// - This method can be called multiple times, but it will only call finalizers for dependencies that were resolved since the last call
        ///
        /// - If the container has a parent, it will also close the parent container if [`Self::close_parent`] is set to `true`
        #[inline]
        pub fn close(&self) {
            self.inner.lock().close();
        }
    }
}

pub use handle::ContainerHandle;

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    extern crate std;

    use super::{Container, Inject, InjectTransient, RegistriesBuilder};
    use crate::{scope::DefaultScope::*, ContainerHandle, Scope};

    use alloc::{
        boxed::Box,
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
            .provide(|| (Ok(CAAAAA)), Request)
            .provide(|Inject(caaaaa): Inject<CAAAAA>| Ok(CAAAA(caaaaa)), Request)
            .provide(|Inject(caaaa): Inject<CAAAA>| Ok(CAAA(caaaa)), Request)
            .provide(|Inject(caaa): Inject<CAAA>| Ok(CAA(caaa)), Request)
            .provide(|Inject(caa): Inject<CAA>| Ok(CA(caa)), Request)
            .provide(|Inject(ca): Inject<CA>| Ok(C(ca)), Request)
            .provide(|| Ok(B(2)), Request)
            .provide(|Inject(b): Inject<B>, Inject(c): Inject<C>| Ok(A(b, c)), Request);
        let mut container = Container::new(registry);

        let _ = container.get::<A>().unwrap();
        let _ = container.get::<CAAAAA>().unwrap();
        let _ = container.get::<CAAAA>().unwrap();
        let _ = container.get::<CAAA>().unwrap();
        let _ = container.get::<CAA>().unwrap();
        let _ = container.get::<CA>().unwrap();
        let _ = container.get::<C>().unwrap();
        let _ = container.get::<B>().unwrap();
    }

    struct RequestTransient1;
    struct RequestTransient2(RequestTransient1);
    struct RequestTransient3(RequestTransient1, RequestTransient2);

    #[test]
    #[traced_test]
    fn test_transient_get() {
        let registry = RegistriesBuilder::new()
            .provide(|| Ok(RequestTransient1), Runtime)
            .provide(
                |InjectTransient(req): InjectTransient<RequestTransient1>| Ok(RequestTransient2(req)),
                Runtime,
            )
            .provide(
                |InjectTransient(req_1): InjectTransient<RequestTransient1>, InjectTransient(req_2): InjectTransient<RequestTransient2>| {
                    Ok(RequestTransient3(req_1, req_2))
                },
                Runtime,
            );
        let mut container = Container::new(registry);

        container.get_transient::<RequestTransient1>().unwrap();
        container.get_transient::<RequestTransient2>().unwrap();
        container.get_transient::<RequestTransient3>().unwrap();
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

        assert_eq!(runtime_container.parent, None);
        assert_eq!(runtime_container.child_registries.len(), 5);
        assert_eq!(runtime_container.root_registry.scope.priority, Runtime.priority());

        assert_eq!(app_container.parent, Some(Box::new(runtime_container.clone())));
        assert_eq!(app_container.child_registries.len(), 4);
        assert_eq!(app_container.root_registry.scope.priority, App.priority());
        assert!(Arc::ptr_eq(&app_container.root_registry, &runtime_container.child_registries[0]));

        // Session scope is skipped by default, but it is still present in the child registries
        assert_eq!(
            request_container.parent.as_ref().unwrap().root_registry.scope.priority,
            Session.priority()
        );
        assert_eq!(request_container.child_registries.len(), 2);
        assert_eq!(request_container.root_registry.scope.priority, Request.priority());
        // Session scope is skipped by default, so it is not the first child registry
        assert!(Arc::ptr_eq(&request_container.root_registry, &app_container.child_registries[1]));

        assert_eq!(action_container.parent, Some(Box::new(request_container.clone())));
        assert_eq!(action_container.child_registries.len(), 1);
        assert_eq!(action_container.root_registry.scope.priority, Action.priority());
        assert!(Arc::ptr_eq(&action_container.root_registry, &request_container.child_registries[0]));

        assert_eq!(step_container.parent, Some(Box::new(action_container.clone())));
        assert_eq!(step_container.child_registries.len(), 0);
        assert_eq!(step_container.root_registry.scope.priority, Step.priority());
        assert!(Arc::ptr_eq(&step_container.root_registry, &action_container.child_registries[0]));
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

        assert_eq!(runtime_container.parent, None);
        assert_eq!(runtime_container.child_registries.len(), 5);
        assert_eq!(runtime_container.root_registry.scope.priority, Runtime.priority());

        assert_eq!(app_container.parent, Some(Box::new(runtime_container.clone())));
        assert_eq!(app_container.child_registries.len(), 4);
        assert_eq!(app_container.root_registry.scope.priority, App.priority());
        assert!(Arc::ptr_eq(&app_container.root_registry, &runtime_container.child_registries[0]));

        assert_eq!(session_container.parent, Some(Box::new(app_container.clone())));
        assert_eq!(session_container.child_registries.len(), 3);
        assert_eq!(session_container.root_registry.scope.priority, Session.priority());
        assert!(Arc::ptr_eq(&session_container.root_registry, &app_container.child_registries[0]));

        assert_eq!(request_container.parent, Some(Box::new(session_container.clone())));
        assert_eq!(request_container.child_registries.len(), 2);
        assert_eq!(request_container.root_registry.scope.priority, Request.priority());
        assert!(Arc::ptr_eq(
            &request_container.root_registry,
            &session_container.child_registries[0]
        ));

        assert_eq!(action_container.parent, Some(Box::new(request_container.clone())));
        assert_eq!(action_container.child_registries.len(), 1);
        assert_eq!(action_container.root_registry.scope.priority, Action.priority());
        assert!(Arc::ptr_eq(&action_container.root_registry, &request_container.child_registries[0]));

        assert_eq!(step_container.parent, Some(Box::new(action_container.clone())));
        assert_eq!(step_container.child_registries.len(), 0);
        assert_eq!(step_container.root_registry.scope.priority, Step.priority());
        assert!(Arc::ptr_eq(&step_container.root_registry, &action_container.child_registries[0]));
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

        let mut runtime_container = Container::new(registry);
        let mut app_container = runtime_container.clone().enter().with_scope(App).build().unwrap();
        let mut request_container = app_container.clone().enter().with_scope(Request).build().unwrap();

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
        let app_container = runtime_container.enter().with_scope(App).build().unwrap();
        let mut request_container = app_container.enter().with_scope(Request).build().unwrap();

        let _ = request_container.get::<()>().unwrap();
        let _ = request_container.get::<((), ())>().unwrap();
        let _ = request_container.get::<((), (), (), (), ())>().unwrap();
        let _ = request_container.get::<((), (), (), ())>().unwrap();

        let runtime_container_resolved_set_count = request_container
            .parent
            .as_ref()
            .unwrap()
            .parent
            .as_ref()
            .unwrap()
            .context
            .get_resolved_set()
            .0
            .len();
        let app_container_resolved_set_count = request_container.parent.as_ref().unwrap().context.get_resolved_set().0.len();
        let request_container_resolved_set_count = request_container.context.get_resolved_set().0.len();

        request_container.close();

        assert_eq!(runtime_container_resolved_set_count, 1);
        assert_eq!(app_container_resolved_set_count, 1);
        assert_eq!(request_container_resolved_set_count, 2);

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
    fn test_shared_container_close_for_resolved() {
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

                    debug!("Finalizer 1 called");
                }
            })
            .add_finalizer({
                let finalizer_2_request_call_count = finalizer_2_request_call_count.clone();
                move |_: Arc<((), ())>| {
                    finalizer_2_request_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Finalizer 2 called");
                }
            })
            .add_finalizer({
                let finalizer_3_request_call_count = finalizer_3_request_call_count.clone();
                move |_: Arc<((), (), (), ())>| {
                    finalizer_3_request_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Finalizer 3 called");
                }
            });

        let runtime_container = Container::new(registry).shared();
        let app_container = runtime_container.enter().with_scope(App).build().unwrap().shared();
        let request_container = app_container.enter().with_scope(Request).build().unwrap().shared();

        let _ = request_container.get::<()>().unwrap();
        let _ = request_container.get::<((), ())>().unwrap();
        let _ = request_container.get::<((), (), (), ())>().unwrap();

        request_container.close();

        assert_eq!(finalizer_1_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_2_request_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(finalizer_3_request_call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_bounds() {
        fn impl_bounds<T: Send + Sync + 'static>() {}

        impl_bounds::<Container>();
        impl_bounds::<ContainerHandle>();
    }
}
