use alloc::boxed::Box;
use core::{future::Future, pin::Pin};

use super::base::Service;

pub(crate) struct BoxCloneServiceSync<Request: ?Sized, Response, Error>(
    pub(crate)  Box<
        dyn CloneService<
            Request,
            Response = Response,
            Error = Error,
            Output = Result<Response, Error>,
        >,
    >,
);

pub(crate) type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

pub(crate) struct BoxCloneServiceAsync<Request: ?Sized, Response, Error>(
    Box<
        dyn CloneService<
            Request,
            Response = Response,
            Error = Error,
            Output = BoxFuture<Result<Response, Error>>,
        >,
    >,
);

pub(crate) trait CloneService<Request: ?Sized>: Service<Request> {
    #[must_use]
    fn clone_box(
        &self,
    ) -> Box<
        dyn CloneService<
            Request,
            Response = Self::Response,
            Error = Self::Error,
            Output = Self::Output,
        >,
    >;
}

impl<Request, T> CloneService<Request> for T
where
    Request: ?Sized,
    T: Service<Request> + Clone + ?Sized + 'static,
{
    #[inline]
    #[must_use]
    fn clone_box(
        &self,
    ) -> Box<dyn CloneService<Request, Response = T::Response, Error = T::Error, Output = T::Output>>
    {
        Box::new(self.clone())
    }
}

impl<Request: ?Sized, Response, Error> Clone for BoxCloneServiceSync<Request, Response, Error> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl<Request: ?Sized, Response, Error> Clone for BoxCloneServiceAsync<Request, Response, Error> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl<Request, Response, Error> Service<Request> for BoxCloneServiceSync<Request, Response, Error> {
    type Response = Response;
    type Error = Error;
    type Output = Result<Response, Error>;

    #[inline]
    fn call(&mut self, request: Request) -> Self::Output {
        self.0.call(request)
    }
}

impl<Request, Response, Error> Service<Request> for BoxCloneServiceAsync<Request, Response, Error> {
    type Response = Response;
    type Error = Error;
    type Output = BoxFuture<Result<Response, Error>>;

    #[inline]
    fn call(&mut self, request: Request) -> Self::Output {
        self.0.call(request)
    }
}
