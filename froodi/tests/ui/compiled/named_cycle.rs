use froodi::compiled_registry as registry;
use froodi::{Container, DefaultScope::App, Inject, InstantiateErrorKind};

struct Greeter;
struct WelcomeHandler;

fn greeter(_: Inject<WelcomeHandler>) -> Result<Greeter, InstantiateErrorKind> {
    Ok(Greeter)
}

fn welcome_handler(_: Inject<Greeter>) -> Result<WelcomeHandler, InstantiateErrorKind> {
    Ok(WelcomeHandler)
}

fn main() {
    let fragment = registry! { provide(App, welcome_handler) };
    let _ = Container::new(registry! {
        provide(App, greeter),
        extend(fragment),
    });
}
