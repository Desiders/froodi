//! Public syntax templates from a different crate; declarations may live in modules.
pub use froodi::{instance, DefaultScope::App, Inject, InstantiateErrorKind};

#[derive(Clone)]
pub struct Settings(pub u32);

pub struct Repository(pub u32);

pub fn make_repository(Inject(settings): Inject<Settings>) -> Result<Repository, InstantiateErrorKind> {
    Ok(Repository(settings.0 + 1))
}

pub mod first {
    /// Registers shared settings and a repository.
    #[froodi::fragment(pub infrastructure(settings))]
    froodi::registry! {
        use crate::fragments::{instance, make_repository, App};
        provide(App, instance(settings)),
        provide(App, make_repository),
    }
}

pub mod second {
    #[froodi::fragment(pub infrastructure(value))]
    froodi::registry! {
        use crate::fragments::{instance, App};
        provide(App, instance(value)),
    }
}

#[allow(dead_code)]
fn private_provider() -> Result<u64, InstantiateErrorKind> {
    Ok(7)
}

#[froodi::fragment(pub private_body)]
froodi::registry! {
    use crate::fragments::App;
    provide(App, crate::fragments::private_provider),
}

#[froodi::fragment(private_fragment)]
froodi::registry! {}
