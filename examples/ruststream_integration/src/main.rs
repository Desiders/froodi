use froodi::{
    Container,
    DefaultScope::{App, Request},
    Inject, boxed, instance, registry,
    ruststream::ContainerLayer,
};
use ruststream::memory::prelude::*;
use ruststream::runtime::{Identity, Stack};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc};

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

/// The message the service consumes and, at startup, publishes to itself.
#[derive(Outgoing, Serialize, Deserialize)]
#[outgoing(name = "greetings")]
struct Greeting {
    name: String,
}

#[subscriber("greetings")]
async fn greet(greeting: &Greeting, Inject(welcome): Inject<WelcomeHandler>) -> HandlerOutcome {
    println!("{}", welcome.handle(&greeting.name));
    HandlerOutcome::ack()
}

#[ruststream::app]
fn app() -> RustStream<Stack<ContainerLayer, Identity>> {
    let app_container = build_container(Config {
        greeting: "Hello".to_owned(),
    });
    let root = app_container.clone();

    RustStream::new(AppInfo::new("greetings", "0.1.0"))
        .layer(ContainerLayer::new(app_container))
        .after_shutdown(move |_state| async move {
            root.close();
            Ok::<_, Infallible>(())
        })
        .with_broker(MemoryBroker::new(), |b| {
            b.include(greet);
            b.after_startup(Publish, async move |publisher| {
                publisher
                    .message(&Greeting {
                        name: "ruststream".to_owned(),
                    })
                    .publish()
                    .await
            });
        })
}
