use froodi::{
    DefaultScope::{App, Request},
    Inject, InstantiatorResult,
    async_impl::{Container, RegistriesBuilder},
    instance,
};
use std::sync::Arc;

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
        println!("User created");
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
    async fn create_user<R>(Inject(repo): Inject<R>) -> InstantiatorResult<CreateUser<R>> {
        Ok(CreateUser { repo })
    }

    // We can use functions as instance finalizer instead of closures
    async fn finalize_create_user<R>(_dep: Arc<CreateUser<R>>) {
        println!("Create user interactor finalized");
    }

    let registry = RegistriesBuilder::new()
        // We still can use sync instance creator even with async container
        .provide(instance(config), App)
        .provide(|_config: Inject<Config>| Ok(PostgresUserRepo), Request)
        // We can specify async instance creator using `provide_async` method instead of `provide`
        .provide_async(create_user::<PostgresUserRepo>, Request)
        // We still can use sync instance finalizer even with async container
        .add_finalizer::<PostgresUserRepo>(|_dep| println!("Postgres repository finalized"))
        // We can specify async instance finalizer using `add_async_finalizer` method instead of `add_finalizer`
        .add_async_finalizer(finalize_create_user::<PostgresUserRepo>)
        .add_finalizer::<Config>(|_dep| println!("Config finalized"));
    Container::new(registry)
}

// Output:
// User created
// Create user interactor finalized
// Postgres repository finalized
// Request container finalized
// Config finalized
// App container finalized
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let app_container = init_container(Config::default());
    // Enter the container with next non-skipped scope (APP -> REQUEST -> ..., check default scope variants).
    // Don't worry about cloning because it's free.
    let request_container = app_container.clone().enter_build().unwrap();

    // Get REQUEST-scoped dependency from REQUEST-scoped container
    let interactor = request_container.get::<CreateUser<PostgresUserRepo>>().await.unwrap();
    interactor.handle();

    // Get APP-scoped dependency from REQUEST-scoped container.
    // We can use dependencies from previous containers.
    let _config = request_container.get::<Config>().await.unwrap();

    // We need to close containers after usage of them to call finalizers of cached dependencies.
    // It will close only REQUEST-scoped and SESSION-scoped dependencies.
    request_container.close().await;
    println!("Request container finalized");

    // It will close only APP-scoped and RUNTIME-scoped dependencies.
    app_container.close().await;
    println!("App container finalized");
}
