use dptree::Endpoint;
use froodi::{
    Container,
    DefaultScope::Request,
    InjectTransient, InstantiatorResult,
    dptree::{Injectable, MapInject, setup_default},
    registry,
};
use std::ops::ControlFlow;

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

fn init_container() -> Container {
    // We can use functions as instance creators instead of closures
    #[allow(clippy::unnecessary_wraps)]
    fn create_user<R>(InjectTransient(repo): InjectTransient<R>) -> InstantiatorResult<CreateUser<R>> {
        Ok(CreateUser { repo })
    }

    Container::new(registry! {
        scope(Request) [
            provide(|| Ok(PostgresUserRepo)),
            provide(create_user::<PostgresUserRepo>),
        ],
    })
}

fn init_branch(container: Container, config: Config) -> Endpoint<'static, ()> {
    dptree::filter_map(move || Some(config.clone()))
        .filter_map_async(
            // We need to register this function to inject the container.
            // You can use `MapInject` in next endpoints only for values from previous factories (`filter_map`).
            setup_default(container),
        )
        .endpoint(
            // We need to wrap the handler into `Injectable` struct to inject its args from the container
            Injectable::new(handler),
        )
}

async fn handler(
    // Get REQUEST-scoped dependency from REQUEST-scoped container
    InjectTransient(interactor): InjectTransient<CreateUser<PostgresUserRepo>>,
    // Get dependency from dptree's dependency map
    MapInject(_config): MapInject<Config>,
    // We also can inject container itself
    InjectTransient(_request_container): InjectTransient<Container>,
) {
    interactor.handle();
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let container = init_container();

    let handler = dptree::entry().branch(init_branch(container, Config::default()));
    let result = handler.dispatch(dptree::deps![]).await;

    assert_eq!(result, ControlFlow::Break(()));
}
