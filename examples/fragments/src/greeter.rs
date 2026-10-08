use froodi::registry;

pub(super) trait Greeter: Send + Sync {
    fn greet(&self, name: &str) -> String;
}

pub(super) struct GreetingService {
    pub(super) greeting: String,
}

impl Greeter for GreetingService {
    fn greet(&self, name: &str) -> String {
        format!("{}, {name}!", self.greeting)
    }
}

#[froodi::fragment(pub(crate) registrations(cfg))]
registry! {
    use froodi::{
        DefaultScope::{App, Request},
        Inject, boxed, instance,
    };

    use crate::{
        Config,
        greeter::{Greeter, GreetingService},
    };

    provide(App, instance(cfg)),
    scope(Request) [
        provide(|Inject(cfg): Inject<Config>| Ok(boxed!(GreetingService { greeting: cfg.greeting.clone() }; Greeter))),
    ],
}
