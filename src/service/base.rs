pub trait Service<Request: ?Sized> {
    type Response;
    type Error;

    fn call(&mut self, request: Request) -> Result<Self::Response, Self::Error>;
}

impl<'a, S: Service<Request> + 'a + ?Sized, Request> Service<Request> for &'a mut S {
    type Response = S::Response;
    type Error = S::Error;

    #[inline]
    fn call(&mut self, request: Request) -> Result<Self::Response, Self::Error> {
        (**self).call(request)
    }
}
