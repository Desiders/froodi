use froodi::{
    Container,
    DefaultScope::{App, Request},
    Inject, InjectTransient, InstantiatorResult, instance, registry,
    telers::setup_default,
};
use telers::{
    Bot, Dispatcher, Extension, Router,
    enums::UpdateType,
    event::{EventReturn, telegram::HandlerResult},
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
        provide(App, instance(config)),
        scope(Request) [
            provide(|_config: Inject<Config>| Ok(PostgresUserRepo)),
            provide(create_user::<PostgresUserRepo>),
        ],
    })
}

async fn handler(
    // Get REQUEST-scoped dependency from REQUEST-scoped container
    InjectTransient(interactor): InjectTransient<CreateUser<PostgresUserRepo>>,
    // We also can inject container itself using `Extension` or `Inject`/`InjectTransient`
    Extension(_request_container): Extension<Container>,
) -> HandlerResult {
    interactor.handle();

    Ok(EventReturn::Finish)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let app_container = init_container(Config::default());

    let bot = Bot::from_env_by_key("BOT_TOKEN");

    let mut router = Router::new("main");
    router.message.register(handler);

    let router = setup_default(router, app_container.clone());

    let dispatcher = Dispatcher::builder()
        .main_router(router.configure_default())
        .bot(bot)
        .allowed_update(UpdateType::Message)
        .build();

    dispatcher.run_polling().await.unwrap();

    app_container.close();
}
