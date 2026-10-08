extern crate std;

use alloc::{vec, vec::Vec};

use std::{cell::RefCell, rc::Rc};

use crate::{registry, utils::thread_safety::RcThreadSafety, Container, DefaultScope::*, Inject, InstantiateErrorKind};

#[derive(Clone)]
struct Config(&'static str);
struct Database(Rc<Config>);

fn make_database(Inject(config): Inject<Config>) -> Result<Database, InstantiateErrorKind> {
    Ok(Database(config))
}

#[test]
fn shares_values_through_rc_and_finalizes_on_close() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let finalizer = {
        let log = log.clone();
        move |_: RcThreadSafety<Database>| log.borrow_mut().push("database")
    };
    let container = Container::new(registry! {
        scope(App) [
            provide(|| Ok::<_, InstantiateErrorKind>(Config("local"))),
            provide(make_database, finalizer = finalizer),
        ],
    });

    let database = container.get::<Database>().unwrap();
    assert!(Rc::ptr_eq(&database.0, &container.get::<Config>().unwrap()));
    assert_eq!(container.get_transient::<Config>().unwrap().0, "local");

    container.close();
    assert_eq!(*log.borrow(), vec!["database"]);
}
