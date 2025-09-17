use alloc::boxed::Box;
use core::future::Future;

use super::MapFuture;

pub trait Service<Request: ?Sized> {
    type Response;
    type Error;
    type Future: Future<Output = Result<Self::Response, Self::Error>>;

    fn call(&mut self, request: Request) -> Self::Future;
}

impl<'a, S: Service<Request> + 'a + ?Sized, Request> Service<Request> for &'a mut S {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn call(&mut self, request: Request) -> Self::Future {
        (**self).call(request)
    }
}

impl<S, Request> Service<Request> for Box<S>
where
    S: Service<Request> + ?Sized,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&mut self, request: Request) -> S::Future {
        (**self).call(request)
    }
}

pub trait ServiceExt<Request>: Service<Request> {
    fn map_future<F, Fut, Response, Error>(self, f: F) -> MapFuture<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Future) -> Fut,
        Error: From<Self::Error>,
        Fut: Future<Output = Result<Response, Error>>,
    {
        MapFuture::new(self, f)
    }
}

impl<T: ?Sized, Request> ServiceExt<Request> for T where T: Service<Request> {}
