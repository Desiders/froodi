use froodi::{
    Container, Context,
    DefaultScope::{App, Request},
    Inject, InstantiateErrorKind, declare, instance, registry,
};

#[derive(Clone)]
struct AppSettings {
    greeting: String,
}

struct RequestId(u64);

struct Greeting(String);

fn make_greeting(Inject(settings): Inject<AppSettings>, Inject(request_id): Inject<RequestId>) -> Result<Greeting, InstantiateErrorKind> {
    Ok(Greeting(format!("{} from request {}", settings.greeting, request_id.0)))
}

fn main() {
    let app = Container::new(registry! {
        provide(Request, declare::<RequestId>()),
        provide(Request, make_greeting),
        extend(registry! {
            provide(App, instance(AppSettings {
                greeting: "Hello".to_owned(),
            })),
        }),
    });

    for request_id in [42, 43] {
        let mut context = Context::new();
        context.insert(RequestId(request_id));

        let request = app.clone().enter().with_scope(Request).with_context(context).build().unwrap();
        let greeting = request.get::<Greeting>().unwrap();
        println!("{}", greeting.0);
        request.close();
    }

    app.close();
}
