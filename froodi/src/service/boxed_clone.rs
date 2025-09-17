use alloc::boxed::Box;

use crate::utils::thread_safety::{SendSafety, SyncSafety};

use super::base::Service;

#[cfg(feature = "thread_safe")]
pub(crate) type BoxCloneServiceInner<Request, Response, Error> =
    Box<dyn CloneService<Request, Response = Response, Error = Error> + Send + Sync>;

#[cfg(not(feature = "thread_safe"))]
pub(crate) type BoxCloneServiceInner<Request, Response, Error> = Box<dyn CloneService<Request, Response = Response, Error = Error>>;

pub(crate) struct BoxCloneService<Request: ?Sized, Response, Error>(pub(crate) BoxCloneServiceInner<Request, Response, Error>);

pub(crate) trait CloneService<Request: ?Sized>: Service<Request> {
    #[must_use]
    fn clone_box(&self) -> BoxCloneServiceInner<Request, Self::Response, Self::Error>;
}

impl<Request, T> CloneService<Request> for T
where
    Request: ?Sized,
    T: Service<Request> + SendSafety + SyncSafety + Clone + 'static,
{
    #[inline]
    fn clone_box(&self) -> BoxCloneServiceInner<Request, T::Response, T::Error> {
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

    #[inline]
    fn call(&mut self, request: Request) -> Result<Self::Response, Self::Error> {
        self.0.call(request)
    }
}
