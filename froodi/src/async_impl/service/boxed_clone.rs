use alloc::boxed::Box;

use super::base::{Service, ServiceExt as _};
use crate::utils::{
    future::BoxFuture,
    thread_safety::{SendSafety, SyncSafety},
};

#[cfg(feature = "thread_safe")]
pub(crate) type BoxCloneServiceInner<Request, Response, Error, Future = BoxFuture<'static, Result<Response, Error>>> =
    Box<dyn CloneService<Request, Response = Response, Error = Error, Future = Future> + Send + Sync>;

#[cfg(not(feature = "thread_safe"))]
pub(crate) type BoxCloneServiceInner<Request, Response, Error, Future = BoxFuture<'static, Result<Response, Error>>> =
    Box<dyn CloneService<Request, Response = Response, Error = Error, Future = Future>>;

pub(crate) struct BoxCloneService<Request: ?Sized, Response, Error>(pub(crate) BoxCloneServiceInner<Request, Response, Error>);

impl<Request, Response, Error> BoxCloneService<Request, Response, Error> {
    pub fn new<S>(inner: S) -> Self
    where
        S: Service<Request, Response = Response, Error = Error> + SendSafety + SyncSafety + Clone + 'static,
        S::Future: SendSafety + 'static,
    {
        BoxCloneService(Box::new(inner.map_future(|f| Box::pin(f) as _)))
    }
}

pub(crate) trait CloneService<Request: ?Sized>: Service<Request> {
    #[must_use]
    fn clone_box(&self) -> BoxCloneServiceInner<Request, Self::Response, Self::Error, Self::Future>;
}

impl<Request, T> CloneService<Request> for T
where
    Request: ?Sized,
    T: Service<Request> + SendSafety + SyncSafety + Clone + 'static,
{
    #[inline]
    fn clone_box(&self) -> BoxCloneServiceInner<Request, T::Response, T::Error, T::Future> {
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
