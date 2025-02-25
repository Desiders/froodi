use alloc::boxed::Box;

use super::base::Service;

pub(crate) struct BoxCloneService<Request: ?Sized, Response, Error>(
    pub(crate) Box<dyn CloneService<Request, Response = Response, Error = Error>>,
);

pub(crate) trait CloneService<Request: ?Sized>: Service<Request> {
    #[must_use]
    fn clone_box(&self) -> Box<dyn CloneService<Request, Response = Self::Response, Error = Self::Error>>;
}

impl<Request, T> CloneService<Request> for T
where
    Request: ?Sized,
    T: Service<Request> + Clone + 'static,
{
    #[inline]
    #[must_use]
    fn clone_box(&self) -> Box<dyn CloneService<Request, Response = T::Response, Error = T::Error>> {
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
