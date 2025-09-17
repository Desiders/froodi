use alloc::boxed::Box;

use super::base::{Service, ServiceExt as _};
use crate::utils::future::BoxFuture;

pub(crate) struct BoxCloneService<Request: ?Sized, Response, Error>(
    pub(crate)  Box<
        dyn CloneService<Request, Response = Response, Error = Error, Future = BoxFuture<'static, Result<Response, Error>>> + Send + Sync,
    >,
);

impl<Request, Response, Error> BoxCloneService<Request, Response, Error> {
    pub fn new<S>(inner: S) -> Self
    where
        S: Service<Request, Response = Response, Error = Error> + Clone + Send + Sync + 'static,
        S::Future: Send + 'static,
    {
        BoxCloneService(Box::new(inner.map_future(|f| Box::pin(f) as _)))
    }
}

pub(crate) trait CloneService<Request: ?Sized>: Service<Request> {
    #[must_use]
    fn clone_box(
        &self,
    ) -> Box<dyn CloneService<Request, Response = Self::Response, Error = Self::Error, Future = Self::Future> + Send + Sync>;
}

impl<Request, T> CloneService<Request> for T
where
    Request: ?Sized,
    T: Service<Request> + Clone + Send + Sync + 'static,
{
    #[inline]
    fn clone_box(&self) -> Box<dyn CloneService<Request, Response = T::Response, Error = T::Error, Future = T::Future> + Send + Sync> {
        Box::new(self.clone())
    }
}

impl<Request: ?Sized, Response, Error> Clone for BoxCloneService<Request, Response, Error> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl<Request, Response, Error> Service<Request> for BoxCloneService<Request, Response, Error> {
    type Response = Response;
    type Error = Error;
    type Future = BoxFuture<'static, Result<Response, Error>>;

    #[inline]
    fn call(&mut self, request: Request) -> Self::Future {
        self.0.call(request)
    }
}
