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

struct RequestId(usize);

#[derive(froodi::Construct)]
struct WelcomeHandler {
    #[di(inject)]
    greeter: Arc<Box<dyn Greeter>>,
    #[di(inject_transient)]
    request_id: RequestId,
}

impl WelcomeHandler {
    fn handle(&self, name: &str) {
        println!("Request {}: {}", self.request_id.0, self.greeter.greet(name));
    }
}

fn build_container(cfg: Config) -> Container {
    Container::new(registry! {
        provide(App, instance(cfg)),
        scope(Request) [
            provide(|Inject(cfg): Inject<Config>| Ok(boxed!(GreetingService { greeting: cfg.greeting.clone() }; Greeter))),
            provide(|| Ok(RequestId(123))),
            provide::<WelcomeHandler>(),
        ],
    })
}

fn main() {
    let cfg = Config {
        greeting: "Hello".to_owned(),
    };

    let app_container = build_container(cfg);
    let request_container = app_container.clone().enter_build().expect("Failed to enter request scope");

    let handler = request_container
        .get_transient::<WelcomeHandler>()
        .expect("WelcomeHandler not registered");

    handler.handle("froodi");

    request_container.close();
    app_container.close();
}
