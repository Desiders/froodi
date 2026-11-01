#![allow(unused)]

use froodi::{Construct, Container, Context, DefaultScope::Request, declare, registry};

struct RequestId(i64);

#[derive(Construct)]
struct Service {
    #[di(inject_transient)]
    req: RequestId,
}

fn main() {
    let container = Container::new(registry! {
        scope(Request) [
            provide(declare::<RequestId>()),
            provide::<Service>()
        ]
    });

    for request_id in [1, 2, 3] {
        let mut context = Context::new();
        context.insert(RequestId(request_id));

        let container = container.clone().enter().with_context(context).build().unwrap();

        let _service = container.get::<Service>().unwrap();

        container.close();
    }

    container.close();
}
