use alloc::rc::Rc;
use core::cell::RefCell;

use super::{context::Context, dependency_resolver::DependencyResolver, registry::Registry};

pub(crate) struct Container {
    context: Rc<RefCell<Context>>,
    registry: Rc<Registry>,
}

impl Container {
    pub(crate) fn get<Dep: DependencyResolver>(&self) -> Result<Dep, Dep::Error> {
        Dep::resolve(self.registry.clone(), self.context.clone())
    }
}
