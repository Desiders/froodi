pub trait Service<Request: ?Sized> {
    type Response;
    type Error;

    // It can be used for call of sync/async versions.
    // In case of sync we can use `Result<Request, Response>`, but for async `Future<Output = Result<Request, Response>`.
    type Output;

    fn call(&mut self, request: Request) -> Self::Output;
}

impl<'a, S: Service<Request> + 'a + ?Sized, Request> Service<Request> for &'a mut S {
    type Response = S::Response;
    type Error = S::Error;
    type Output = S::Output;

    #[inline]
    fn call(&mut self, request: Request) -> Self::Output {
        (**self).call(request)
    }
}
