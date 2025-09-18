use core::future::Future;

use super::Service;

#[derive(Clone)]
pub struct MapFuture<S, F> {
    inner: S,
    f: F,
}

impl<S, F> MapFuture<S, F> {
    pub const fn new(inner: S, f: F) -> Self {
        Self { inner, f }
    }
}

impl<R, S, F, T, E, Fut> Service<R> for MapFuture<S, F>
where
    S: Service<R>,
    F: FnMut(S::Future) -> Fut,
    E: From<S::Error>,
    Fut: Future<Output = Result<T, E>>,
{
    type Response = T;
    type Error = E;
    type Future = Fut;

    fn call(&mut self, req: R) -> Self::Future {
        (self.f)(self.inner.call(req))
    }
}
