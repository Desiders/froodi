#![allow(unused)]

use froodi::{
    Construct,
    DefaultScope::{App, Request},
    Inject, InjectTransient,
    async_impl::Container,
    boxed, registry,
};
use std::sync::Arc;

#[derive(Construct)]
struct APIClient;

struct Conn;

trait UserDao: Send + Sync {}

struct MemoryUserDao {
    conn: Arc<Conn>,
}

impl UserDao for MemoryUserDao {}

struct Service {
    client: Arc<APIClient>,
    dao: Box<dyn UserDao>,
}

#[tokio::main]
async fn main() {
    let registry = registry! {
        scope(App) [
            provide::<APIClient>()
        ],
        scope(Request) [
            provide(async || Ok(Conn)),
            provide(async |Inject(conn)| Ok(boxed!(MemoryUserDao { conn }; UserDao))),
            provide(async |Inject(client), InjectTransient(dao)| Ok(Service { client, dao })),
        ]
    };
    let container = Container::new_with_start_scope(registry, Request);

    let _service = container.get_transient::<Service>().await.unwrap();

    container.close().await;
}
