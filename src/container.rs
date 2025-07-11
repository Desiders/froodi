use alloc::{boxed::Box, rc::Rc};
use core::cell::RefCell;

use super::{context::Context, dependency_resolver::DependencyResolver, registry::RegistriesBuilder};
use crate::{
    dependency_resolver::{Inject, InjectTransient},
    errors::{ResolveErrorKind, ScopeErrorKind, ScopeWithErrorKind},
    registry::Registry,
    scope::Scope,
};

#[derive(Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Container {
    context: Rc<RefCell<Context>>,
    root_registry: Rc<Registry>,
    child_registries: Box<[Rc<Registry>]>,
    parent: Option<Box<Container>>,
}

#[cfg(feature = "eq")]
impl PartialEq for Container {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.root_registry, &other.root_registry)
            && Rc::ptr_eq(&self.context, &other.context)
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
            context: Rc::new(RefCell::new(Context::new())),
            root_registry,
            child_registries,
            parent: None,
        }
    }

    /// Creates child container with next non-skipped scope.
    ///
    /// # Errors
    /// - Returns [ScopeErrorKind::NoChildRegistries] if there are no registries
    /// - Returns [ScopeErrorKind::NoNonSkippedRegistries] if there are no non-skipped registries
    ///
    /// # Warning
    /// - This method skips skipped scopes, if you want to use one of them, use [Self::scope_with]
    /// - If you want to use specific scope, use [Self::scope_with]
    pub fn scope(&self) -> Result<Container, ScopeErrorKind> {
        use ScopeErrorKind::*;

        let mut iter = self.child_registries.iter();
        let registry = iter.next().ok_or(NoChildRegistries)?;

        let mut child = self.child(registry.clone(), iter.cloned().collect());
        while child.root_registry.scope.is_skipped_by_default {
            let mut iter = child.child_registries.iter();
            let registry = iter.next().ok_or(NoNonSkippedRegistries)?;

            child = child.child(registry.clone(), iter.cloned().collect());
        }

        Ok(child)
    }

    /// Creates child container with specified scope.
    ///
    /// # Errors
    /// - Returns [ScopeWithErrorKind::NoChildRegistries] if there are no registries
    /// - Returns [ScopeWithErrorKind::NoChildRegistriesWithScope] if there are no registries with specified scope
    ///
    /// # Warning
    /// If you want just to use next non-skipped scope, use [Self::scope]
    pub fn scope_with<S: Scope>(&self, scope: S) -> Result<Container, ScopeWithErrorKind> {
        use ScopeWithErrorKind::*;

        let priority = scope.priority();

        let mut iter = self.child_registries.iter();
        let registry = iter.next().ok_or(NoChildRegistries)?;

        let mut child = self.child(registry.clone(), iter.cloned().collect());
        while child.root_registry.scope.priority != priority {
            let mut iter = child.child_registries.iter();
            let registry = iter.next().ok_or(NoChildRegistriesWithScope {
                name: scope.name(),
                priority,
            })?;

            child = child.child(registry.clone(), iter.cloned().collect());
        }

        Ok(child)
    }

    pub fn get<Dep: 'static>(&self) -> Result<Rc<Dep>, ResolveErrorKind> {
        match Inject::resolve(self.root_registry.clone(), self.context.clone()).map(|Inject(dep)| dep) {
            Ok(dep) => Ok(dep),
            Err(err @ ResolveErrorKind::NoInstantiator) => match &self.parent {
                Some(parent) => parent.get(),
                None => Err(err),
            },
            Err(err) => Err(err),
        }
    }

    pub fn get_transient<Dep: 'static>(&self) -> Result<Dep, ResolveErrorKind> {
        match InjectTransient::resolve(self.root_registry.clone(), self.context.clone()).map(|InjectTransient(dep)| dep) {
            Ok(dep) => Ok(dep),
            Err(err @ ResolveErrorKind::NoInstantiator) => match &self.parent {
                Some(parent) => parent.get_transient(),
                None => Err(err),
            },
            Err(err) => Err(err),
        }
    }
}

impl Container {
    fn child(&self, root_registry: Rc<Registry>, child_registries: Box<[Rc<Registry>]>) -> Container {
        Container {
            context: self.context.clone(),
            root_registry,
            child_registries,
            parent: Some(Box::new(self.clone())),
        }
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
        let container = Container::new(registry);

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
        let container = Container::new(registry);

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
        let app_container = runtime_container.scope().unwrap();
        let request_container = app_container.scope().unwrap();
        let action_container = request_container.scope().unwrap();
        let step_container = action_container.scope().unwrap();

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
        let app_container = runtime_container.scope_with(App).unwrap();
        let session_container = runtime_container.scope_with(Session).unwrap();
        let request_container = app_container.scope_with(Request).unwrap();
        let action_container = request_container.scope_with(Action).unwrap();
        let step_container = action_container.scope_with(Step).unwrap();

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
}
