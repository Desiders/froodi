use froodi::{Construct, registry};
use std::sync::Arc;

use crate::{client::APIClient, database::UserDao};

#[derive(Construct)]
pub struct Service {
    #[di(inject)]
    client: Arc<APIClient>,
    #[di(inject_transient)]
    dao: Box<dyn UserDao>,
}

#[froodi::fragment(pub(crate) registrations)]
registry! {
    use froodi::DefaultScope::Request;

    use crate::service::Service;

    scope(Request) [
        provide::<Service>()
    ]
}
