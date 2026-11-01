#![allow(unused)]

use froodi::{Container, DefaultScope::Request, registry};

use crate::service::Service;

mod client;
mod database;
mod service;

fn main() {
    let registry = registry! {
        extend_fragment(
            client::registrations!(),
            database::registrations!(),
            service::registrations!(),
        ),
    };
    let container = Container::new_with_start_scope(registry, Request);

    let _service = container.get_transient::<Service>().unwrap();

    container.close();
}
