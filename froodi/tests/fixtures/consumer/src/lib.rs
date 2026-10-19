use std::sync::Arc;

#[derive(di::Construct)]
pub struct DerivedService {
    #[di(inject)]
    number: Arc<u32>,
    #[di(inject_transient)]
    fresh: u32,
}

impl DerivedService {
    pub fn number(&self) -> u32 {
        *self.number
    }

    pub fn fresh(&self) -> u32 {
        self.fresh
    }
}

#[derive(di::Construct)]
pub struct GenericService<T> {
    value: Arc<T>,
    #[di(inject_transient)]
    fresh: T,
}

impl<T> GenericService<T> {
    pub fn values(&self) -> (&T, &T) {
        (&self.value, &self.fresh)
    }
}
