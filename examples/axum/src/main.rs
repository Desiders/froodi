use axum::{Extension, Router, routing::get};
use froodi::{
    Container,
    DefaultScope::{App, Request},
    Inject, InstantiatorResult, RegistryBuilder,
    axum::setup_default,
    instance,
};
use std::sync::Arc;
use tokio::net::TcpListener;

// Dependency that will be alive throughout the application
#[derive(Default, Clone)]
struct Config {
    _host: &'static str,
    _port: i16,
    _user: &'static str,
    _password: &'static str,
    _db: &'static str,
}

trait UserRepo {
    fn create_user(&self);
}

struct PostgresUserRepo;

impl UserRepo for PostgresUserRepo {
    fn create_user(&self) {
        todo!()
    }
}

struct CreateUser<R> {
    // Dependency without details about the specific implementation.
    // It's inside `Arc` because of caching and finalization features.
    repo: Arc<R>,
}

impl<R: UserRepo> CreateUser<R> {
    fn handle(&self) {
        self.repo.create_user();
    }
}

fn init_container(config: Config) -> Container {
    // We can use functions as instance creators instead of closures
    #[allow(clippy::unnecessary_wraps)]
    fn create_user<R>(Inject(repo): Inject<R>) -> InstantiatorResult<CreateUser<R>> {
        Ok(CreateUser { repo })
    }

    let registry = RegistryBuilder::new()
        .provide(instance(config), App)
        .provide(|_config: Inject<Config>| Ok(PostgresUserRepo), Request)
        .provide(create_user::<PostgresUserRepo>, Request);
    Container::new(registry)
}

async fn handler(
    // Get REQUEST-scoped dependency from REQUEST-scoped container
    Inject(interactor): Inject<CreateUser<PostgresUserRepo>>,
    // We also can inject container itself using `Extension` or `Inject`/`InjectTransient`
    Extension(_request_container): Extension<Container>,
) {
    interactor.handle();
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let app_container = init_container(Config::default());

    let router = Router::new().route("/", get(handler));
    let router = setup_default(router, app_container.clone());
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, router).await.unwrap();

    app_container.close();
}
