use dptree::Endpoint;
use froodi::{
    Container,
    DefaultScope::{App, Request},
    Inject, boxed,
    dptree::{Injectable, MapInject, setup_default},
    instance, registry,
};
use std::ops::ControlFlow;
use std::sync::Arc;

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

fn init_branch(container: Container, name: String) -> Endpoint<'static, ()> {
    dptree::filter_map(move || Some(name.clone()))
        .filter_map_async(setup_default(container))
        .endpoint(Injectable::new(handler))
}

async fn handler(Inject(handler): Inject<WelcomeHandler>, MapInject(name): MapInject<String>) {
    println!("{}", handler.handle(&name));
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let container = build_container(Config {
        greeting: "Hello".to_owned(),
    });

    let handler = dptree::entry().branch(init_branch(container, "dptree".to_owned()));
    let result = handler.dispatch(dptree::deps![]).await;

    assert_eq!(result, ControlFlow::Break(()));
}
