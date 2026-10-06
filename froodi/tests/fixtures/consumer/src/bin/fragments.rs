use di::{
    fragment, instance, registry, Container, DefaultScope::App, Inject, InstantiateErrorKind, TypedContainer, TypedContainerExt as _,
};
use scope_support::fragments::{first::infrastructure as infrastructure_alias, second, Repository, Settings};

struct Handler(u32);

fn make_handler(Inject(repository): Inject<Repository>) -> Result<Handler, InstantiateErrorKind> {
    Ok(Handler(repository.0))
}

mod application {
    #[di::fragment(pub(crate) handlers)]
    di::registry! {
        use crate::{make_handler, App};
        provide(App, make_handler),
    }
}

mod configuration {
    #[di::fragment(pub infrastructure(value))]
    di::registry! {
        use crate::{App, instance};
        provide(App, instance(value)),
    }
}

// A syntax fragment can import another exported fragment and retains its body scope.
#[fragment(outer(settings))]
registry! {
    use scope_support::fragments::first::infrastructure;
    extend_fragment(infrastructure!(settings)),
}

#[fragment(module_alias(settings))]
registry! {
    use scope_support::fragments::first::{self as infrastructure};
    extend_fragment(infrastructure::infrastructure!(settings)),
}

#[fragment(inherited_import(settings))]
registry! {
    extend_fragment(enclosing_alias!(settings)),
}

#[cfg(not(any()))]
use scope_support::fragments::first::infrastructure as conditional_fragment;
#[cfg(any())]
use scope_support::fragments::second::infrastructure as conditional_fragment;

fn main() {
    let container = TypedContainer::new(registry! {
        extend_fragment(outer!(Settings(41)), application::handlers!(), second::infrastructure!(17u64)),
        provide(App, instance(true)),
    });
    assert_eq!(container.get::<Handler>().unwrap().0, 42);
    assert_eq!(*container.get::<u64>().unwrap(), 17);
    let container = Container::new(registry! {
        extend_fragment(infrastructure_alias!(Settings(41)), configuration::infrastructure!(7u16)),
    });
    assert_eq!(container.get::<Repository>().unwrap().0, 42);
    assert_eq!(*container.get::<u16>().unwrap(), 7);

    let container = Container::new(registry! {
        extend_fragment(module_alias!(Settings(41))),
    });
    assert_eq!(container.get::<Repository>().unwrap().0, 42);

    let container = Container::new(registry! {
        use scope_support::fragments::first::infrastructure as enclosing_alias;
        extend_fragment(inherited_import!(Settings(41))),
    });
    assert_eq!(container.get::<Repository>().unwrap().0, 42);

    let container = Container::new(registry! {
        extend_fragment(conditional_fragment!(Settings(41))),
    });
    assert_eq!(container.get::<Repository>().unwrap().0, 42);

    mod glob {
        pub use scope_support::fragments::first::*;
    }
    let erased = registry! { extend_fragment(glob::infrastructure!(Settings(41))) }.into_registry();
    let container = Container::new(registry! { extend(erased) });
    assert_eq!(container.get::<Repository>().unwrap().0, 42);
}
