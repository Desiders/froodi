use froodi::{
    Container,
    DefaultScope::{App, Request},
    Inject, boxed, instance, registry,
    telers::setup_default,
};
use std::sync::Arc;
use telers::{
    Bot, Dispatcher, Router,
    enums::UpdateType,
    event::telegram::{Handler, HandlerResult},
};

trait Greeter: Send + Sync {
    fn greet(&self, name: &str) -> String;
}

#[derive(Clone)]
struct Config {
    greeting: String,
}

struct GreetingService {
    greeting: String,
}

impl Greeter for GreetingService {
    fn greet(&self, name: &str) -> String {
        format!("{}, {name}!", self.greeting)
    }
}

struct WelcomeHandler {
    greeter: Arc<Box<dyn Greeter>>,
}

impl WelcomeHandler {
    fn handle(&self, name: &str) -> String {
        self.greeter.greet(name)
    }
}

fn build_container(cfg: Config) -> Container {
    Container::new(registry! {
        provide(App, instance(cfg)),
        scope(Request) [
            provide(|Inject(cfg): Inject<Config>| Ok(boxed!(GreetingService { greeting: cfg.greeting.clone() }; Greeter))),
            provide(|Inject(greeter)| Ok(WelcomeHandler { greeter })),
        ],
    })
}

async fn handler(Inject(handler): Inject<WelcomeHandler>) -> HandlerResult<()> {
    println!("{}", handler.handle("telers"));
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let app_container = build_container(Config {
        greeting: "Hello".to_owned(),
    });

    let bot = Bot::from_env();

    let router = setup_default(Router::new("main"), app_container.clone()).on_message(|observer| observer.register(Handler::new(handler)));

    let dispatcher = Dispatcher::builder()
        .main_router(router.configure_default())
        .bot(bot)
        .allowed_update(UpdateType::Message)
        .build();

    dispatcher.run_polling().await.unwrap();

    app_container.close();
}
