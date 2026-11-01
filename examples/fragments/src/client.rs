use froodi::{Construct, registry};

#[derive(Construct)]
pub struct APIClient;

#[froodi::fragment(pub(crate) registrations)]
registry! {
    use froodi::DefaultScope::App;

    use crate::client::APIClient;

    scope(App) [
        provide::<APIClient>()
    ]
}
