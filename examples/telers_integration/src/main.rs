#![allow(unused)]

use froodi::{
    Construct,
    DefaultScope::{App, Request},
    Inject,
    async_impl::Container,
    boxed, registry,
    telers::setup_async_default,
};
use std::sync::Arc;
use telers::{
    Bot, Dispatcher, Router,
    enums::UpdateType,
    event::telegram::{Handler, HandlerResult},
};

#[derive(Construct)]
struct APIClient;

#[derive(Construct)]
struct Conn;

trait UserDao: Send + Sync {}

struct MemoryUserDao {
    conn: Arc<Conn>,
}

impl UserDao for MemoryUserDao {}

#[derive(Construct)]
struct Service {
    #[di(inject)]
    client: Arc<APIClient>,
    #[di(inject_transient)]
    dao: Box<dyn UserDao>,
}

async fn handler(Inject(_service): Inject<Service>) -> HandlerResult<()> {
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let registry = registry! {
        scope(App) [
            provide::<APIClient>()
        ],
        scope(Request) [
            provide::<Conn>(),
            provide(|Inject(conn)| Ok(boxed!(MemoryUserDao { conn }; UserDao))),
            provide::<Service>(),
        ]
    };
    let container = Container::new(registry);

    let bot = Bot::from_env();

    let router =
        setup_async_default(Router::new("main"), container.clone()).on_message(|observer| observer.register(Handler::new(handler)));

    let dispatcher = Dispatcher::builder()
        .main_router(router.configure_default())
        .bot(bot)
        .allowed_update(UpdateType::Message)
        .build();

    dispatcher.run_polling().await.unwrap();

    container.close().await;
}
