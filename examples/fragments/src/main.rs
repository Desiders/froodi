use froodi::{Container, registry};

use crate::welcome::WelcomeHandler;

mod greeter;
mod welcome;

#[derive(Clone)]
struct Config {
    greeting: String,
}

fn build_container(cfg: Config) -> Container {
    Container::new(registry! {
        extend_fragment(
            greeter::registrations!(cfg),
            welcome::registrations!(),
        ),
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
