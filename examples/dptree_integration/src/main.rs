#![allow(unused)]

use dptree::Endpoint;
use froodi::{
    Construct, Container,
    DefaultScope::{App, Request},
    Inject, boxed,
    dptree::{Injectable, MapInject, setup_default},
    registry,
};
use std::ops::ControlFlow;
use std::sync::Arc;

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

fn init_branch(container: Container, name: String) -> Endpoint<'static, ()> {
    dptree::filter_map(move || Some(name.clone()))
        .filter_map_async(setup_default(container))
        .endpoint(Injectable::new(handler))
}

async fn handler(Inject(_service): Inject<Service>, MapInject(_name): MapInject<String>) {}

#[tokio::main(flavor = "current_thread")]
async fn main() {
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

    let handler = dptree::entry().branch(init_branch(container.clone(), "dptree".to_owned()));
    let result = handler.dispatch(dptree::deps![]).await;

    assert_eq!(result, ControlFlow::Break(()));

    container.close();
}
