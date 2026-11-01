#![allow(unused)]

use froodi::{
    Construct, Container,
    DefaultScope::{App, Request},
    Inject, boxed, registry,
    ruststream::ContainerLayer,
};
use ruststream::memory::prelude::*;
use ruststream::runtime::{Identity, Stack};
use serde::Deserialize;
use std::{convert::Infallible, sync::Arc};

#[derive(Construct)]
struct APIClient;

#[derive(Construct)]
struct Conn;

trait UserDao: Send + Sync {}

struct MemoryUserDao {
    conn: Arc<Conn>,
}

impl UserDao for MemoryUserDao {}

#[derive(Construct)]
struct Service {
    #[di(inject)]
    client: Arc<APIClient>,
    #[di(inject_transient)]
    dao: Box<dyn UserDao>,
}

#[derive(Deserialize)]
struct Req;

#[subscriber("handler")]
async fn handler(_req: &Req, Inject(_service): Inject<Service>) -> HandlerOutcome {
    HandlerOutcome::ack()
}

#[ruststream::app]
fn app() -> RustStream<Stack<ContainerLayer, Identity>> {
    let registry = registry! {
        scope(App) [
            provide::<APIClient>()
        ],
        scope(Request) [
            provide::<Conn>(),
            provide(|Inject(conn)| Ok(boxed!(MemoryUserDao { conn }; UserDao))),
            provide::<Service>(),
        ]
    };
    let container = Container::new(registry);

    RustStream::new(AppInfo::new("greetings", "0.1.0"))
        .layer(ContainerLayer::new(container.clone()))
        .after_shutdown(move |_state| async move {
            container.close();
            Ok::<_, Infallible>(())
        })
        .with_broker(MemoryBroker::new(), |b| {
            b.include(handler);
        })
}
