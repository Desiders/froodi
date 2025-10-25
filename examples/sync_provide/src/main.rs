use froodi::{
    Container,
    DefaultScope::{App, Request},
    Inject, InjectTransient, InstantiatorResult, instance, registry,
};

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
    // Dependency without details about the specific implementation
    repo: R,
}

impl<R: UserRepo> CreateUser<R> {
    fn handle(&self) {
        self.repo.create_user();
    }
}

fn init_container(config: Config) -> Container {
    // We can use functions as instance creators instead of closures
    #[allow(clippy::unnecessary_wraps)]
    fn create_user<R>(InjectTransient(repo): InjectTransient<R>) -> InstantiatorResult<CreateUser<R>> {
        Ok(CreateUser { repo })
    }

    Container::new(registry! {
        scope(App) [
            provide(instance(config)),
        ],
        scope(Request) [
            provide(|_config: Inject<Config>| Ok(PostgresUserRepo)),
            provide(create_user::<PostgresUserRepo>),
        ],
    })
}

fn main() {
    let app_container = init_container(Config::default());
    // Enter the container with next non-skipped scope (APP -> REQUEST -> ..., check default scope variants).
    // Don't worry about cloning because it's free.
    let request_container = app_container.clone().enter_build().unwrap();

    // Get REQUEST-scoped dependency from REQUEST-scoped container
    let interactor = request_container.get_transient::<CreateUser<PostgresUserRepo>>().unwrap();
    interactor.handle();

    // Get APP-scoped dependency from REQUEST-scoped container.
    // We can use dependencies from previous containers.
    let _config = request_container.get::<Config>().unwrap();

    // We need to close containers after usage of them.
    // Currently, it's not necessary, but we usually need to call finalizers of cached dependencies when we close. Check finalizer example.
    request_container.close();
    app_container.close();
}
