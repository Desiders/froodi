use super::base::Service;

#[derive(Clone)]
pub(crate) struct FnService<F>(pub(crate) F);

impl<F, Request, Response, Error> Service<Request> for FnService<F>
where
    F: FnMut(Request) -> Result<Response, Error>,
{
    type Response = Response;
    type Error = Error;

    #[inline]
    fn call(&mut self, request: Request) -> Result<Self::Response, Self::Error> {
        self.0(request)
    }
}

#[cfg(test)]
mod tests {
    use core::convert::Infallible;

    use super::{FnService, Service as _};

    #[derive(Clone, Copy)]
    struct Request(bool);
    struct Response(bool);

    #[test]
    fn test_service() {
        let mut service = FnService(|Request(val)| Ok::<_, Infallible>(Response(val)));

        let request = Request(true);
        let response = service.call(request).unwrap();

        assert_eq!(request.0, response.0);
    }
}
