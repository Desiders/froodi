/// Builds a typed registry for [`Container`](crate::Container).
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
#[macro_export]
macro_rules! registry {
    ($($tokens:tt)*) => {
        $crate::macros_utils::typed::registry!($crate::macros_utils::typed; $($tokens)*)
    };
}

/// Builds a typed asynchronous registry for [`async_impl::Container`](crate::async_impl::Container).
///
/// Uses the same `provide`, `scope` and `extend` syntax as [`registry!`]. The `async`
/// feature is required. `.into_async_registry()` explicitly erases a typed fragment.
#[cfg(feature = "async")]
#[macro_export]
macro_rules! async_registry {
    ($($tokens:tt)*) => {
        $crate::macros_utils::typed::async_registry!($crate::macros_utils::typed; $($tokens)*)
    };
}
