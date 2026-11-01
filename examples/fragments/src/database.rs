use std::sync::Arc;

use froodi::{Construct, registry};

#[derive(Construct)]
pub struct Conn;

pub trait UserDao: Send + Sync {}

pub struct MemoryUserDao {
    pub conn: Arc<Conn>,
}

impl UserDao for MemoryUserDao {}

#[froodi::fragment(pub(crate) registrations)]
registry! {
    use froodi::{
        DefaultScope::Request,
        Inject, boxed,
    };

    use crate::database::{Conn, MemoryUserDao, UserDao};

    scope(Request) [
        provide::<Conn>(),
        provide(|Inject(conn)| Ok(boxed!(MemoryUserDao { conn }; UserDao)))
    ]
}
