use super::base::Service;

#[inline]
#[must_use]
pub(crate) const fn service_fn<T>(f: T) -> ServiceFn<T> {
    ServiceFn { f }
}

#[derive(Clone)]
pub(crate) struct ServiceFn<T> {
    f: T,
}

impl<F, Request, Response, Error> Service<Request> for ServiceFn<F>
where
    F: FnMut(Request) -> Result<Response, Error>,
{
    type Response = Response;
    type Error = Error;

    #[inline]
    fn call(&mut self, request: Request) -> Result<Self::Response, Self::Error> {
        (self.f)(request)
    }
}

#[cfg(test)]
mod tests {
    use core::convert::Infallible;

    use super::{service_fn, Service as _};

    #[derive(Clone, Copy)]
    struct Request(bool);
    struct Response(bool);

    #[test]
    fn test_service() {
        let mut service = service_fn(|Request(val)| Ok::<_, Infallible>(Response(val)));

        let request = Request(true);
        let response = service.call(request).unwrap();

        assert_eq!(request.0, response.0);
    }
}
