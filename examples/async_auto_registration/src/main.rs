use froodi::{
    DefaultScope::{App, Request},
    Inject, InstantiateErrorKind,
    async_impl::Container,
    async_registry, instance, registry,
};
use froodi_auto::{AutoRegistriesWithSync, injectable};
use std::sync::Arc;

#[derive(Clone)]
struct Database {
    url: &'static str,
}

struct UserRepository {
    db: Arc<Database>,
}

#[injectable]
impl UserRepository {
    #[provide(Request)]
    async fn new(Inject(db): Inject<Database>) -> Result<Self, InstantiateErrorKind> {
        Ok(Self { db })
    }

    fn find(&self, id: u64) -> String {
        format!("user_{id} from {}", self.db.url)
    }
}

struct StartHandler {
    repo: Arc<UserRepository>,
}

#[injectable]
impl StartHandler {
    #[provide(Request)]
    async fn new(Inject(repo): Inject<UserRepository>) -> Result<Self, InstantiateErrorKind> {
        Ok(Self { repo })
    }

    fn handle(&self, user_id: u64) {
        println!("Welcome, {}!", self.repo.find(user_id));
    }
}

fn build_container(db: Database) -> Container {
    Container::new(
        async_registry! {
            extend(registry! {
                provide(App, instance(db)),
            })
        }
        .provide_auto_registries_with_sync(),
    )
}

#[tokio::main]
async fn main() {
    let app_container = build_container(Database {
        url: "postgres://localhost/bot",
    });

    let request_container = app_container.clone().enter_build().expect("Failed to enter request scope");

    let handler = request_container
        .get_transient::<StartHandler>()
        .await
        .expect("StartHandler not registered");

    handler.handle(42);

    request_container.close().await;
    app_container.close().await;
}
