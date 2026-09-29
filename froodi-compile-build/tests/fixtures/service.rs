//! A web service's wiring written the way Froodi's examples and tests write it. Parsed by
//! `tests/sample.rs`; it is not compiled.

use std::sync::Arc;

use froodi::{
    async_registry, instance, registry, AsyncRegistry, DefaultScope::{App, Request, Session}, Inject, InjectTransient,
    InstantiateErrorKind, InstantiatorResult, Registry,
};

use crate::{
    db::{connect_pool, make_migrator},
    http::HttpClient,
    telemetry::telemetry_registry,
};

type Db = Arc<Pool>;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub cache_provides: bool,
}
pub struct Clock;
pub struct Cache;
pub struct Pool;
pub struct UserRepository(Db);
pub struct OrderRepository(Arc<Pool>);
pub struct Mailer;
pub struct UserService;
pub struct OrderService;
pub struct Handler;
pub struct AuditLog;

fn make_cache(Inject(config): Inject<Config>) -> InstantiatorResult<Cache> {
    Ok(Cache)
}

fn make_clock() -> Result<Clock, InstantiateErrorKind> {
    Ok(Clock)
}

fn make_user_repository(Inject(pool): Inject<Db>) -> InstantiatorResult<UserRepository> {
    Ok(UserRepository(pool))
}

fn make_order_repository(Inject(pool): Inject<Arc<Pool>>) -> InstantiatorResult<OrderRepository> {
    Ok(OrderRepository(pool))
}

fn make_user_service(
    Inject(users): Inject<UserRepository>,
    Inject(cache): Inject<Cache>,
    InjectTransient(clock): InjectTransient<Clock>,
) -> InstantiatorResult<UserService> {
    Ok(UserService)
}

fn make_handler<S: Service>(Inject(service): Inject<S>) -> InstantiatorResult<Handler> {
    Ok(Handler)
}

fn close_mailer(_mailer: Arc<Mailer>) {}

pub fn registry(config: Config) -> Registry {
    registry! {
        provide(App, instance(config)),
        provide(App, instance(Clock)),
        provide(App, connect_pool),
        scope(Session) [
            provide(make_cache),
            provide(make_clock, config = froodi::Config { cache_provides: false }),
        ],
        scope(Request) [
            provide(make_user_repository),
            provide(make_order_repository),
            provide(make_user_service),
            provide(|Inject(orders): Inject<OrderRepository>, Inject(users): Inject<UserService>| Ok::<_, InstantiateErrorKind>(OrderService)),
            provide(|Inject(config): Inject<Config>| -> InstantiatorResult<Mailer> { Ok(Mailer) }, finalizer = close_mailer),
            provide(|Inject(http): Inject<HttpClient>| Ok(AuditLog)),
            provide(make_handler::<UserService>),
            provide(make_migrator),
        ],
        extend(telemetry_registry(), registry! { provide(App, instance(HttpClient { timeout: 30 })) }),
    }
}

pub async fn async_registry(config: Config) -> AsyncRegistry {
    async_registry! {
        provide(App, |Inject(config): Inject<Config>| async move { Ok::<Pool, InstantiateErrorKind>(Pool) }),
        extend(registry(config)),
    }
}
