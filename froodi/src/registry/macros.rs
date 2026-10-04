/// Builds a typed registry of synchronous and asynchronous factories.
///
/// Registrations use `provide(scope, instantiator)`, optionally with `config` and
/// `finalizer`. `scope(scope) [ ... ]` groups registrations, and `extend(fragment)`
/// composes fragments. Expressions are evaluated once in source order.
///
/// ```rust
/// use froodi::{instance, registry, Container, DefaultScope::App, Inject, InstantiateErrorKind};
///
/// fn make_message(number: Inject<u32>) -> Result<String, InstantiateErrorKind> {
///     Ok(number.0.to_string())
/// }
///
/// let container = Container::new(registry! {
///     provide(App, instance(7u32)),
///     provide(App, make_message),
/// });
/// assert_eq!(&*container.get::<String>().unwrap(), "7");
/// ```
///
/// `Inject<T>` and `InjectTransient<T>` link to a unique static provider. Use
/// [`InjectCustom<T>`](crate::InjectCustom) for an opaque custom resolver. The inferred
/// registry type retains linking information; `.into_registry()` explicitly erases
/// it into a native [`Registry`](crate::Registry) for runtime composition.
///
/// Closed graphs receive compile-time cycle checks when an instantiated constructor
/// is built. Registrations using [`StaticScope`](crate::StaticScope) also receive
/// compile-time scope checks when both ends of an edge are statically known.
/// Runtime-valued scopes and open compositions retain runtime validation.
///
/// With the `async` feature, sync and async factories can appear
/// together. Their Rust types determine how they execute. A sync factory may
/// depend only on sync providers; an async factory may depend on either kind.
/// Sync factories use sync finalizers; async factories use async finalizers.
/// [`Container`](crate::Container) resolves sync factories only; unrelated async
/// factories can coexist. Requesting an async-only provider returns an error.
/// `async_impl::Container` accepts both kinds.
/// Use `.into_registry()` to erase a mixed fragment for runtime composition.
/// Custom instantiator types should implement one execution trait;
/// implementing both kinds leaves their registration kind ambiguous.
#[macro_export]
macro_rules! registry {
    ($($tokens:tt)*) => {
        $crate::macros_utils::typed::registry!($crate::macros_utils::typed; $($tokens)*)
    };
}
