use core::future::Future;

use super::base::Service;

#[derive(Clone)]
pub(crate) struct FnServiceSync<F>(pub(crate) F);

#[derive(Clone)]
pub(crate) struct FnServiceAsync<F>(pub(crate) F);

impl<F, Request, Response, Error> Service<Request> for FnServiceSync<F>
where
    F: FnMut(Request) -> Result<Response, Error>,
{
    type Response = Response;
    type Error = Error;
    type Output = Result<Self::Response, Self::Error>;

    #[inline]
    fn call(&mut self, request: Request) -> Self::Output {
        self.0(request)
    }
}

impl<F, Request, Response, Error, Fut> Service<Request> for FnServiceAsync<F>
where
    F: FnMut(Request) -> Fut,
    Fut: Future<Output = Result<Response, Error>>,
{
    type Response = Response;
    type Error = Error;
    type Output = Fut;

    #[inline]
    fn call(&mut self, request: Request) -> Self::Output {
        self.0(request)
    }
}

#[cfg(test)]
mod tests {
    use core::convert::Infallible;

    use super::{FnServiceAsync, FnServiceSync, Service as _};

    #[derive(Clone, Copy)]
    struct Request(bool);
    struct Response(bool);

    #[test]
    fn test_service_sync() {
        let mut service = FnServiceSync(|Request(val)| Ok::<_, Infallible>(Response(val)));

        let request = Request(true);
        let response = service.call(request).unwrap();

        assert_eq!(request.0, response.0);
    }

    #[tokio::test]
    async fn test_service_async() {
        let mut service =
            FnServiceAsync(|Request(val)| async move { Ok::<_, Infallible>(Response(val)) });

        let request = Request(true);
        let response = service.call(request).await.unwrap();

        assert_eq!(request.0, response.0);
    }
}
