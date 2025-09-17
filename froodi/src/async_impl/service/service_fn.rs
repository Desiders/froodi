use core::future::Future;

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

impl<F, Fut, Request, Response, Error> Service<Request> for ServiceFn<F>
where
    F: FnMut(Request) -> Fut,
    Fut: Future<Output = Result<Response, Error>>,
{
    type Response = Response;
    type Error = Error;
    type Future = Fut;

    #[inline]
    fn call(&mut self, request: Request) -> Self::Future {
        (self.f)(request)
    }
}

#[cfg(test)]
mod tests {
    use super::{service_fn, Service as _};

    use core::convert::Infallible;

    #[derive(Clone, Copy)]
    struct Request(bool);
    struct Response(bool);

    #[tokio::test]
    async fn test_service() {
        let mut service = service_fn(|Request(val)| async move { Ok::<_, Infallible>(Response(val)) });

        let request = Request(true);

        let response = service.call(request).await.unwrap();

        assert_eq!(request.0, response.0);
    }
}
