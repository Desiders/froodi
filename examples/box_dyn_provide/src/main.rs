use froodi::{Container, DefaultScope::Request, Inject, InstantiatorResult, RegistryBuilder, boxed};
use std::sync::Arc;

trait UserRepo: Send + Sync {
    fn create_user(&self);
}

struct PostgresUserRepo;

impl UserRepo for PostgresUserRepo {
    fn create_user(&self) {
        todo!()
    }
}

struct CreateUser {
    // Dependency without details about the specific implementation.
    // It's inside `Arc` because of caching and finalization features.
    repo: Arc<Box<dyn UserRepo>>,
}

impl CreateUser {
    fn handle(&self) {
        self.repo.create_user();
    }
}

fn init_container() -> Container {
    // We can use functions as instance creators instead of closures
    #[allow(clippy::unnecessary_wraps)]
    fn create_user(Inject(repo): Inject<Box<dyn UserRepo>>) -> InstantiatorResult<CreateUser> {
        Ok(CreateUser { repo })
    }

    let registry = RegistryBuilder::new()
        .provide(
            || Ok(boxed!(PostgresUserRepo; UserRepo)), // or just `Ok(Box::new(PostgresUserRepo) as Box<dyn UserRepo>`
            Request,
        )
        .provide(create_user, Request);
    Container::new(registry)
}

fn main() {
    let app_container = init_container();
    // Enter the container with next non-skipped scope (APP -> REQUEST -> ..., check default scope variants).
    // Don't worry about cloning because it's free.
    let request_container = app_container.clone().enter_build().unwrap();

    // Get REQUEST-scoped dependency from REQUEST-scoped container
    let interactor = request_container.get::<CreateUser>().unwrap();
    interactor.handle();

    // We need to close containers after usage of them.
    // Currently, it's not necessary, but we usually need to call finalizers of cached dependencies when we close. Check finalizer example.
    request_container.close();
    app_container.close();
}
