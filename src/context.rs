use alloc::{boxed::Box, rc::Rc};
use core::any::TypeId;

#[derive(Default, Clone)]
pub(crate) struct Context {
    map: Option<Box<any::Map>>,
}

impl Context {
    #[must_use]
    pub const fn new() -> Self {
        Self { map: None }
    }

    pub fn insert<T: 'static>(&mut self, value: T) -> Option<Rc<T>> {
        self.map
            .get_or_insert_with(Box::default)
            .insert(TypeId::of::<T>(), Rc::new(value))
            .and_then(|boxed| boxed.downcast().ok())
    }

    pub fn insert_rc<T: 'static>(&mut self, value: Rc<T>) -> Option<Rc<T>> {
        self.map
            .get_or_insert_with(Box::default)
            .insert(TypeId::of::<T>(), value)
            .and_then(|boxed| boxed.downcast().ok())
    }

    #[must_use]
    pub fn get_ref<T: 'static>(&self) -> Option<&T> {
        self.map
            .as_ref()
            .and_then(|map| map.get(&TypeId::of::<T>()))
            .and_then(|boxed| boxed.downcast_ref())
    }

    #[must_use]
    pub fn get<T: 'static>(&self) -> Option<Rc<T>> {
        self.map
            .as_ref()
            .and_then(|map| map.get(&TypeId::of::<T>()))
            .and_then(|boxed| boxed.clone().downcast().ok())
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.as_ref().map_or(true, |map| map.is_empty())
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.as_ref().map_or(0, |map| map.len())
    }
}

mod any {
    use alloc::{collections::BTreeMap, rc::Rc};
    use core::any::{Any, TypeId};

    pub(super) type Map = BTreeMap<TypeId, Rc<dyn Any>>;
}
