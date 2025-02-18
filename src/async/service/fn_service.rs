pub(crate) use tower::util::{service_fn as fn_service, ServiceFn as FnService};

#[cfg(test)]
mod tests {
    use core::convert::Infallible;
    use tower::Service as _;

    use super::fn_service;

    #[derive(Clone, Copy)]
    struct Request(bool);
    struct Response(bool);

    #[tokio::test]
    async fn test_service() {
        let mut service =
            fn_service(|Request(val)| async move { Ok::<_, Infallible>(Response(val)) });

        let request = Request(true);
        let response = service.call(request).await.unwrap();

        assert_eq!(request.0, response.0);
    }
}
