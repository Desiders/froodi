use alloc::boxed::Box;
use core::any::TypeId;

#[derive(Default, Clone)]
pub(crate) struct Context {
    map: Option<Box<any::AnyMap>>,
}

impl Context {
    #[must_use]
    pub const fn new() -> Self {
        Self { map: None }
    }

    pub fn insert<T: Clone + Send + 'static>(&mut self, value: T) -> Option<T> {
        self.map
            .get_or_insert_with(Box::default)
            .insert(TypeId::of::<T>(), Box::new(value))
            .and_then(|boxed| boxed.into_any().downcast().ok().map(|boxed| *boxed))
    }

    #[must_use]
    pub fn get_ref<T: 'static>(&self) -> Option<&T> {
        self.map
            .as_ref()
            .and_then(|map| map.get(&TypeId::of::<T>()))
            .and_then(|boxed| (**boxed).as_any().downcast_ref())
    }

    #[must_use]
    pub fn get<T: 'static>(&self) -> Option<T> {
        self.map
            .as_ref()
            .and_then(|map| map.get(&TypeId::of::<T>()))
            .and_then(|boxed| {
                boxed
                    .clone_box()
                    .into_any()
                    .downcast()
                    .ok()
                    .map(|boxed| *boxed)
            })
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
    use alloc::{boxed::Box, collections::BTreeMap};
    use core::any::{Any, TypeId};

    pub(super) trait AnyClone: Any {
        #[must_use]
        fn clone_box(&self) -> Box<dyn AnyClone + Send>;

        #[must_use]
        fn as_any(&self) -> &dyn Any;

        #[must_use]
        fn as_any_mut(&mut self) -> &mut dyn Any;

        #[must_use]
        fn into_any(self: Box<Self>) -> Box<dyn Any>;
    }

    pub(super) type AnyMap = BTreeMap<TypeId, Box<dyn AnyClone + Send>>;

    impl<T: Clone + Send + 'static> AnyClone for T {
        #[inline]
        fn clone_box(&self) -> Box<dyn AnyClone + Send> {
            Box::new(self.clone())
        }

        #[inline]
        fn as_any(&self) -> &dyn Any {
            self
        }

        #[inline]
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        #[inline]
        fn into_any(self: Box<Self>) -> Box<dyn Any> {
            self
        }
    }

    impl Clone for Box<dyn AnyClone + Send> {
        #[inline]
        fn clone(&self) -> Self {
            (**self).clone_box()
        }
    }
}
