use alloc::rc::Rc;
use core::cell::RefCell;

use super::{context::Context, dependency_resolver::DependencyResolver, registry::Registry};
use crate::{
    dependency_resolver::{Inject, InjectTransient},
    errors::ResolveErrorKind,
};

pub struct Container {
    context: Rc<RefCell<Context>>,
    registry: Rc<Registry>,
}

impl Container {
    #[inline]
    #[must_use]
    pub fn new(registry: Registry) -> Self {
        Self {
            context: Rc::new(RefCell::new(Context::new())),
            registry: Rc::new(registry),
        }
    }

    pub fn get<Dep: 'static>(&self) -> Result<Rc<Dep>, ResolveErrorKind> {
        Inject::resolve(self.registry.clone(), self.context.clone()).map(|Inject(dep)| dep)
    }

    pub fn get_transient<Dep: 'static>(&self) -> Result<Dep, ResolveErrorKind> {
        InjectTransient::resolve(self.registry.clone(), self.context.clone()).map(|InjectTransient(dep)| dep)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use alloc::{
        format,
        rc::Rc,
        string::{String, ToString as _},
    };
    use tracing_test::traced_test;

    use super::{Container, Inject, InjectTransient, Registry};

    struct Request1;
    struct Request2(Rc<Request1>);
    struct Request3(Rc<Request1>, Rc<Request2>);

    #[test]
    #[traced_test]
    fn test_scoped_single_get() {
        let registry = Registry::new()
            .provide(|| Ok(Request1))
            .provide(|Inject(req): Inject<Request1>| Ok(Request2(req)))
            .provide(|Inject(req_1): Inject<Request1>, Inject(req_2): Inject<Request2>| Ok(Request3(req_1, req_2)));
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
    fn test_transient_single_get() {
        let registry = Registry::new()
            .provide(|| Ok(RequestTransient1))
            .provide(|InjectTransient(req): InjectTransient<RequestTransient1>| Ok(RequestTransient2(req)))
            .provide(
                |InjectTransient(req_1): InjectTransient<RequestTransient1>, InjectTransient(req_2): InjectTransient<RequestTransient2>| {
                    Ok(RequestTransient3(req_1, req_2))
                },
            );
        let container: Container = Container::new(registry);

        container.get_transient::<RequestTransient1>().unwrap();
        container.get_transient::<RequestTransient2>().unwrap();
        container.get_transient::<RequestTransient3>().unwrap();
    }
}
