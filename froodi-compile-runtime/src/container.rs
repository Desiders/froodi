use alloc::vec::Vec;
use core::any::TypeId;

use crate::{
    errors::{ResolveErrorKind, TypeInfo},
    graph::{Entry, Link, Walk},
    registry::Registry,
    thread_safety::{BoxAnyThreadSafety, RcThreadSafety, SendSafety, SyncSafety},
};

/// State shared by every container of one tree: the linked registration tree and its entries.
struct Plan {
    /// Keeps the linked tree alive at a stable address; `root` and the entries point into it.
    _tree: BoxAnyThreadSafety,
    root: *const (),
    /// Sorted by `type_id`.
    entries: Vec<Entry>,
}

// SAFETY: the raw pointers only address the boxed tree owned by the same `Plan`, which is
// `Send + Sync` in thread-safe builds; entries hold plain function pointers.
#[cfg(feature = "thread_safe")]
unsafe impl Send for Plan {}
#[cfg(feature = "thread_safe")]
unsafe impl Sync for Plan {}

#[derive(Clone)]
pub struct Container {
    plan: RcThreadSafety<Plan>,
}

impl Container {
    #[must_use]
    pub fn new<Tree, Links>(registry: Registry<Tree>) -> Self
    where
        Tree: Link<Tree, Links>,
        Tree::Linked: Walk<Tree::Linked> + SendSafety + SyncSafety + 'static,
    {
        let tree = alloc::boxed::Box::new(registry.tree.link());
        let mut entries = Vec::new();
        tree.walk(&mut entries);
        entries.sort_by_key(|entry| entry.type_id);
        let root = core::ptr::from_ref::<Tree::Linked>(&tree).cast();
        Self {
            plan: RcThreadSafety::new(Plan {
                _tree: tree,
                root,
                entries,
            }),
        }
    }

    /// Gets a scoped dependency from the container.
    ///
    /// # Errors
    /// Returns [`ResolveErrorKind`] if nothing provides `Dep` or its construction fails.
    pub fn get<Dep: SendSafety + SyncSafety + 'static>(&self) -> Result<RcThreadSafety<Dep>, ResolveErrorKind> {
        let type_info = TypeInfo::of::<Dep>();
        let plan = &*self.plan;
        let Ok(index) = plan.entries.binary_search_by_key(&TypeId::of::<Dep>(), |entry| entry.type_id) else {
            return Err(ResolveErrorKind::NoInstantiator { type_info });
        };
        let entry = &plan.entries[index];
        // SAFETY: the entry was produced by walking the tree `plan.root` points to.
        let value = unsafe { (entry.construct)(plan.root, entry.item, self)? };
        value.downcast::<Dep>().map_err(|value| ResolveErrorKind::IncorrectType {
            expected: type_info,
            actual: TypeInfo {
                name: "<unknown>",
                id: (*value).type_id(),
            },
        })
    }
}
