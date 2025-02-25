pub(crate) use tower::util::{service_fn, ServiceFn};

#[cfg(test)]
mod tests {
    use core::convert::Infallible;

    use super::service_fn;
    use crate::r#async::service::base::Service as _;

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
