use tracing::debug;

use super::{Container, ContainerInner};
use crate::{cache::Resolved, registry::InstantiatorData, service::Service as _};

impl Container {
    /// Closes the container, calling finalizers for resolved dependencies in LIFO order.
    ///
    /// # Warning
    /// This method can be called multiple times, but it will only call finalizers for dependencies that were resolved since the last call
    #[inline]
    pub fn close(&self) {
        self.inner.close();
    }
}

impl ContainerInner {
    #[inline]
    fn close(&self) {
        self.close_scope();
        if self.close_parent {
            if let Some(parent) = &self.parent {
                parent.close();
                debug!("Parent container closed");
            }
        }
    }

    pub(crate) fn close_scope(&self) {
        let mut resolved_set = { self.cache.write().take_resolved_set() };
        while let Some(Resolved { type_info, dependency }) = resolved_set.0.pop_back() {
            let InstantiatorData { finalizer, .. } = self
                .registry
                .get(&type_info)
                .expect("Instantiator should be present for resolved type");

            if let Some(finalizer) = finalizer {
                let _ = finalizer.clone().call(dependency);
                debug!(%type_info, "Finalizer called");
            }
        }

        self.cache.write().map.clone_from(&self.context.map);
    }
}

impl Drop for ContainerInner {
    fn drop(&mut self) {
        self.close();
        debug!(scope = %self.scope_data, "Container closed on drop");
    }
}
