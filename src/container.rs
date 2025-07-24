use alloc::{boxed::Box, rc::Rc};
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
    root_registry: Rc<Registry>,
    child_registries: Box<[Rc<Registry>]>,
    parent: Option<Box<Container>>,
    close_parent: bool,
}

#[cfg(feature = "eq")]
impl PartialEq for Container {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.root_registry, &other.root_registry)
            && self.context == other.context
            && self.child_registries.len() == other.child_registries.len()
            && self
                .child_registries
                .iter()
                .zip(other.child_registries.iter())
                .all(|(a, b)| Rc::ptr_eq(a, b))
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
            (Rc::new(root_registry), registries.map(Rc::new).collect())
        } else {
            panic!("registries len (is 0) should be >= 1");
        };

        Self {
            context: Context::new(),
            root_registry,
            child_registries,
            parent: None,
            close_parent: true,
        }
    }

    #[inline]
    pub fn set_close_parent(&mut self, close_parent: bool) {
        self.close_parent = close_parent;
    }

    /// Creates child container builder
    #[inline]
    #[must_use]
    pub fn child(&self) -> ChildContainerBuiler {
        ChildContainerBuiler { container: self.clone() }
    }

    /// Creates child container and builds it with next non-skipped scope
    #[inline]
    pub fn child_build(&self) -> Result<Container, ScopeErrorKind> {
        self.child().build()
    }

    pub fn get<Dep: 'static>(&mut self) -> Result<Rc<Dep>, ResolveErrorKind> {
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

    /// Closes the container, calling finalizers for resolved dependencies
    ///
    /// # Warning
    /// This method can be called multiple times, but it will only call finalizers for dependencies that were resolved since the last call
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
    fn init_child_with_context(&self, context: Context, root_registry: Rc<Registry>, child_registries: Box<[Rc<Registry>]>) -> Container {
        Container {
            context,
            root_registry,
            child_registries,
            parent: Some(Box::new(self.clone())),
            close_parent: self.close_parent,
        }
    }

    #[inline]
    #[must_use]
    fn init_child(&self, root_registry: Rc<Registry>, child_registries: Box<[Rc<Registry>]>) -> Container {
        self.init_child_with_context(self.context.child(), root_registry, child_registries)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{Container, Inject, InjectTransient, RegistriesBuilder};
    use crate::{scope::DefaultScope::*, Scope};

    use alloc::{
        boxed::Box,
        format,
        rc::Rc,
        string::{String, ToString as _},
    };
    use core::sync::atomic::{AtomicU8, Ordering};
    use tracing::debug;
    use tracing_test::traced_test;

    struct Request1;
    struct Request2(Rc<Request1>);
    struct Request3(Rc<Request1>, Rc<Request2>);

    #[test]
    #[traced_test]
    fn test_scoped_get() {
        let registry = RegistriesBuilder::new()
            .provide(|| Ok(Request1), Runtime)
            .provide(|Inject(req): Inject<Request1>| Ok(Request2(req)), Runtime)
            .provide(
                |Inject(req_1): Inject<Request1>, Inject(req_2): Inject<Request2>| Ok(Request3(req_1, req_2)),
                Runtime,
            );
        let mut container = Container::new(registry);

        let request_1 = container.get::<Request1>().unwrap();
        let request_2 = container.get::<Request2>().unwrap();
        let request_3 = container.get::<Request3>().unwrap();

        assert!(Rc::ptr_eq(&request_1, &request_2.0));
        assert!(Rc::ptr_eq(&request_1, &request_3.0));
        assert!(Rc::ptr_eq(&request_2, &request_3.1));
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
        let app_container = runtime_container.child_build().unwrap();
        let request_container = app_container.child_build().unwrap();
        let action_container = request_container.child_build().unwrap();
        let step_container = action_container.child_build().unwrap();

        assert_eq!(runtime_container.parent, None);
        assert_eq!(runtime_container.child_registries.len(), 5);
        assert_eq!(runtime_container.root_registry.scope.priority, Runtime.priority());

        assert_eq!(app_container.parent, Some(Box::new(runtime_container.clone())));
        assert_eq!(app_container.child_registries.len(), 4);
        assert_eq!(app_container.root_registry.scope.priority, App.priority());
        assert!(Rc::ptr_eq(&app_container.root_registry, &runtime_container.child_registries[0]));

        // Session scope is skipped by default, but it is still present in the child registries
        assert_eq!(
            request_container.parent.as_ref().unwrap().root_registry.scope.priority,
            Session.priority()
        );
        assert_eq!(request_container.child_registries.len(), 2);
        assert_eq!(request_container.root_registry.scope.priority, Request.priority());
        // Session scope is skipped by default, so it is not the first child registry
        assert!(Rc::ptr_eq(&request_container.root_registry, &app_container.child_registries[1]));

        assert_eq!(action_container.parent, Some(Box::new(request_container.clone())));
        assert_eq!(action_container.child_registries.len(), 1);
        assert_eq!(action_container.root_registry.scope.priority, Action.priority());
        assert!(Rc::ptr_eq(&action_container.root_registry, &request_container.child_registries[0]));

        assert_eq!(step_container.parent, Some(Box::new(action_container.clone())));
        assert_eq!(step_container.child_registries.len(), 0);
        assert_eq!(step_container.root_registry.scope.priority, Step.priority());
        assert!(Rc::ptr_eq(&step_container.root_registry, &action_container.child_registries[0]));
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
        let app_container = runtime_container.child().with_scope(App).build().unwrap();
        let session_container = runtime_container.child().with_scope(Session).build().unwrap();
        let request_container = app_container.child().with_scope(Request).build().unwrap();
        let action_container = request_container.child().with_scope(Action).build().unwrap();
        let step_container = action_container.child().with_scope(Step).build().unwrap();

        assert_eq!(runtime_container.parent, None);
        assert_eq!(runtime_container.child_registries.len(), 5);
        assert_eq!(runtime_container.root_registry.scope.priority, Runtime.priority());

        assert_eq!(app_container.parent, Some(Box::new(runtime_container.clone())));
        assert_eq!(app_container.child_registries.len(), 4);
        assert_eq!(app_container.root_registry.scope.priority, App.priority());
        assert!(Rc::ptr_eq(&app_container.root_registry, &runtime_container.child_registries[0]));

        assert_eq!(session_container.parent, Some(Box::new(app_container.clone())));
        assert_eq!(session_container.child_registries.len(), 3);
        assert_eq!(session_container.root_registry.scope.priority, Session.priority());
        assert!(Rc::ptr_eq(&session_container.root_registry, &app_container.child_registries[0]));

        assert_eq!(request_container.parent, Some(Box::new(session_container.clone())));
        assert_eq!(request_container.child_registries.len(), 2);
        assert_eq!(request_container.root_registry.scope.priority, Request.priority());
        assert!(Rc::ptr_eq(&request_container.root_registry, &session_container.child_registries[0]));

        assert_eq!(action_container.parent, Some(Box::new(request_container.clone())));
        assert_eq!(action_container.child_registries.len(), 1);
        assert_eq!(action_container.root_registry.scope.priority, Action.priority());
        assert!(Rc::ptr_eq(&action_container.root_registry, &request_container.child_registries[0]));

        assert_eq!(step_container.parent, Some(Box::new(action_container.clone())));
        assert_eq!(step_container.child_registries.len(), 0);
        assert_eq!(step_container.root_registry.scope.priority, Step.priority());
        assert!(Rc::ptr_eq(&step_container.root_registry, &action_container.child_registries[0]));
    }

    #[test]
    #[traced_test]
    fn test_close_for_unresolved() {
        let finalizer_1_request_call_count = Rc::new(AtomicU8::new(0));
        let finalizer_2_request_call_count = Rc::new(AtomicU8::new(0));
        let finalizer_3_request_call_count = Rc::new(AtomicU8::new(0));

        let registry = RegistriesBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(((), ())), App)
            .provide(|| Ok(((), (), (), ())), Request)
            .add_finalizer({
                let finalizer_1_request_call_count = finalizer_1_request_call_count.clone();
                move |_: Rc<()>| {
                    finalizer_1_request_call_count.fetch_add(1, Ordering::SeqCst);
                }
            })
            .add_finalizer({
                let finalizer_2_request_call_count = finalizer_2_request_call_count.clone();
                move |_: Rc<((), ())>| {
                    finalizer_2_request_call_count.fetch_add(1, Ordering::SeqCst);
                }
            })
            .add_finalizer({
                let finalizer_3_request_call_count = finalizer_3_request_call_count.clone();
                move |_: Rc<((), (), (), ())>| {
                    finalizer_3_request_call_count.fetch_add(1, Ordering::SeqCst);
                }
            });

        let mut runtime_container = Container::new(registry);
        let mut app_container = runtime_container.child().with_scope(App).build().unwrap();
        let mut request_container = app_container.child().with_scope(Request).build().unwrap();

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
        let request_call_count = Rc::new(AtomicU8::new(0));

        let finalizer_1_request_call_count = Rc::new(AtomicU8::new(0));
        let finalizer_1_request_call_position = Rc::new(AtomicU8::new(0));
        let finalizer_2_request_call_count = Rc::new(AtomicU8::new(0));
        let finalizer_2_request_call_position = Rc::new(AtomicU8::new(0));
        let finalizer_3_request_call_count = Rc::new(AtomicU8::new(0));
        let finalizer_3_request_call_position = Rc::new(AtomicU8::new(0));
        let finalizer_4_request_call_count = Rc::new(AtomicU8::new(0));
        let finalizer_4_request_call_position = Rc::new(AtomicU8::new(0));

        let registry = RegistriesBuilder::new()
            .provide(|| Ok(()), Runtime)
            .provide(|| Ok(((), ())), App)
            .provide(|| Ok(((), (), (), ())), Request)
            .provide(|| Ok(((), (), (), (), ())), Request)
            .add_finalizer({
                let request_call_count = request_call_count.clone();
                let finalizer_1_request_call_position = finalizer_1_request_call_position.clone();
                let finalizer_1_request_call_count = finalizer_1_request_call_count.clone();
                move |_: Rc<()>| {
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
                move |_: Rc<((), ())>| {
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
                move |_: Rc<((), (), (), ())>| {
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
                move |_: Rc<((), (), (), (), ())>| {
                    request_call_count.fetch_add(1, Ordering::SeqCst);
                    finalizer_4_request_call_position.store(request_call_count.load(Ordering::SeqCst), Ordering::SeqCst);
                    finalizer_4_request_call_count.fetch_add(1, Ordering::SeqCst);

                    debug!("Finalizer 4 called");
                }
            });

        let runtime_container = Container::new(registry);
        let app_container = runtime_container.child().with_scope(App).build().unwrap();
        let mut request_container = app_container.child().with_scope(Request).build().unwrap();

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
}
