#![allow(unused)]

use axum::{Router, routing::get};
use froodi::{
    Construct,
    DefaultScope::{App, Request},
    Inject,
    async_impl::Container,
    axum::setup_async_default,
    boxed, registry,
};
use std::sync::Arc;
use tokio::net::TcpListener;

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

async fn handler(Inject(_service): Inject<Service>) {}

#[tokio::main]
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

    let router = Router::new().route("/", get(handler));
    let router = setup_async_default(router, container.clone());
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, router).await.unwrap();

    container.close().await;
}
