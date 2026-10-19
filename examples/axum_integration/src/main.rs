use axum::{Router, extract::Path, routing::get};
use froodi::{
    Container,
    DefaultScope::{App, Request},
    Inject,
    axum::setup_default,
    boxed, instance, registry,
};
use std::sync::Arc;
use tokio::net::TcpListener;

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

async fn handler(Path(name): Path<String>, Inject(handler): Inject<WelcomeHandler>) -> String {
    handler.handle(&name)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let app_container = build_container(Config {
        greeting: "Hello".to_owned(),
    });

    let router = Router::new().route("/{name}", get(handler));
    let router = setup_default(router, app_container.clone());
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, router).await.unwrap();

    app_container.close();
}
