#![allow(unused)]

use froodi::{
    Construct,
    DefaultScope::{App, Request},
    Inject,
    async_impl::Container,
    boxed, registry,
};
use std::sync::Arc;

#[derive(Construct)]
struct APIClient;

#[derive(Construct)]
#[di(async)]
struct Conn;

trait UserDao: Send + Sync {}

struct MemoryUserDao {
    conn: Arc<Conn>,
}

impl UserDao for MemoryUserDao {}

#[derive(Construct)]
#[di(async)]
struct Service {
    #[di(inject)]
    client: Arc<APIClient>,
    #[di(inject_transient)]
    dao: Box<dyn UserDao>,
}

#[tokio::main]
async fn main() {
    let registry = registry! {
        provide::<APIClient>(App),
        scope(Request) [
            provide::<Conn>(),
            provide(async |Inject(conn)| Ok(boxed!(MemoryUserDao { conn }; UserDao))),
            provide::<Service>(),
        ]
    };
    let container = Container::new_with_start_scope(registry, Request);

    let _service = container.get_transient::<Service>().await.unwrap();

    container.close().await;
}
