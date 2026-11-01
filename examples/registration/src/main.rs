#![allow(unused)]

use froodi::{
    Construct, Container,
    DefaultScope::{App, Request},
    Inject, boxed, registry,
};
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

fn main() {
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
    let container = Container::new_with_start_scope(registry, Request);

    let _service = container.get_transient::<Service>().unwrap();

    container.close();
}
