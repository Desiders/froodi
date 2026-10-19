use froodi::{
    Container,
    DefaultScope::{App, Request},
    Inject, boxed, instance, registry,
};
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
    fn handle(&self, name: &str) {
        println!("{}", self.greeter.greet(name));
    }
}

fn build_container(cfg: Config) -> Container {
    Container::new(registry! {
        provide(App, instance(cfg), finalizer = |_dep| println!("Config finalized")),
        scope(Request) [
            provide(
                |Inject(cfg): Inject<Config>| Ok(boxed!(GreetingService { greeting: cfg.greeting.clone() }; Greeter)),
                finalizer = |_dep| println!("Greeter finalized")
            ),
            provide(
                |Inject(greeter)| Ok(WelcomeHandler { greeter }),
                finalizer = |_dep| println!("WelcomeHandler finalized")
            ),
        ],
    })
}

fn main() {
    let cfg = Config {
        greeting: "Hello".to_owned(),
    };

    let app_container = build_container(cfg);
    let request_container = app_container.clone().enter_build().expect("Failed to enter request scope");

    let handler = request_container.get::<WelcomeHandler>().expect("WelcomeHandler not registered");

    handler.handle("froodi");

    request_container.close();
    println!("Request container finalized");

    app_container.close();
    println!("App container finalized");
}
