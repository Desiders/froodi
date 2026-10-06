use froodi::registry;
use std::sync::Arc;

use crate::greeter::Greeter;

pub(super) struct RequestId(pub(super) usize);

#[derive(froodi::Construct)]
pub(super) struct WelcomeHandler {
    #[di(inject)]
    greeter: Arc<Box<dyn Greeter>>,
    #[di(inject_transient)]
    request_id: RequestId,
}

impl WelcomeHandler {
    pub(super) fn handle(&self, name: &str) {
        println!("Request {}: {}", self.request_id.0, self.greeter.greet(name));
    }
}

#[froodi::fragment(pub(crate) registrations)]
registry! {
    use froodi::DefaultScope::Request;

    use crate::welcome::{RequestId, WelcomeHandler};

    scope(Request) [
        provide(|| Ok(RequestId(123))),
        provide::<WelcomeHandler>(),
    ],
}
