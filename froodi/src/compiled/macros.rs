/// The `compiled_registry!` macro is used to create a typed dependency registry with various configuration options.
///
/// Requires the `compiled` feature. Use the result with [`Container::new`](crate::Container::new).
///
/// ### `provide` syntax
///
/// Each `provide` item defines a single dependency registration.
/// The following forms are supported:
///
/// ```no_code
/// provide(inst)                             // instantiator only
/// provide(inst, config = Config::default()) // with configuration
/// provide(inst, finalizer = fin)            // with finalizer
/// provide(inst, config = Config::default(), finalizer = fin) // with both parameters
/// provide(inst, finalizer = fin, config = Config::default()) // order doesn’t matter
/// ```
///
/// Parameters:
/// - `config` *(optional)* — configuration object.
/// - `finalizer` *(optional)* — function called when the dependency is finalized.
///
/// ## Usage patterns
///
/// ### 1. Single `scope`
/// ```rust
/// use froodi::{compiled_registry, InstantiateErrorKind, DefaultScope::*};
///
/// fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// compiled_registry! {
///     scope(App) [
///         provide(inst),
///     ]
/// };
/// ```
///
/// ### 2. Multiple `scope`
/// ```rust
/// use froodi::{compiled_registry, InstantiateErrorKind, DefaultScope::*};
///
/// fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// compiled_registry! {
///     scope(App) [ provide(inst) ],
///     scope(Session) [ provide(inst) ],
/// };
/// ```
///
/// ### 3. Single `provide`
/// ```rust
/// use froodi::{compiled_registry, InstantiateErrorKind, DefaultScope::*};
///
/// fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// compiled_registry! {
///     provide(App, inst)
/// };
/// ```
///
/// ### 4. Multiple `provide`
/// ```rust
/// use froodi::{compiled_registry, InstantiateErrorKind, DefaultScope::*};
///
/// fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// compiled_registry! {
///     provide(App, inst),
///     provide(Session, inst),
///     provide(Request, inst),
/// };
/// ```
///
/// ### 5. Combination of one or more `scope` and `provide`
/// ```rust
/// use froodi::{compiled_registry, InstantiateErrorKind, DefaultScope::*};
///
/// fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// compiled_registry! {
///     scope(App) [ provide(inst) ],
///     provide(Session, inst),
///     provide(Request, inst),
/// };
/// ```
///
/// ### 6. Using `extend` standalone
/// ```rust
/// use froodi::compiled_registry;
///
/// compiled_registry! {
///     extend(compiled_registry!())
/// };
/// ```
///
/// ### 7. Using `extend` together with a combination of `scope` and `provide`
/// ```rust
/// use froodi::{compiled_registry, InstantiateErrorKind, DefaultScope::*};
///
/// fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// compiled_registry! {
///     scope(App) [ provide(inst) ],
///     provide(Session, inst),
///     extend(compiled_registry!(), compiled_registry!()),
/// };
/// ```
///
/// ### 8. Empty macro usage
/// ```rust
/// use froodi::compiled_registry;
///
/// let registry = compiled_registry!();
/// ```
/// This creates an empty typed fragment; final construction adds the implicit container registration.
///
/// ## Compiled-specific behavior
///
/// - `Inject<T>` and `InjectTransient<T>` parameters require an unambiguous typed provider.
///   Missing or ambiguous providers fail when the registry is linked.
/// - `.into_registry()` converts the result to [`Registry`](crate::Registry) for returning
///   from functions or further composition. Final cycle/scope validation remains deferred.
/// - Custom resolver parameters use [`RuntimeDependency<T>`](crate::RuntimeDependency).
///   Native fragments and Context values use [`runtime::<T>()`](crate::runtime) /
///   [`context::<T>()`](crate::context) declarations; these declarations supply no value.
/// - Native fragments override typed registrations regardless of their clause position.
///   Later native fragments override earlier native fragments.
/// - `extend(...)` may appear between registrations; expressions evaluate once in source order.
/// - Closed cycle checks run for instantiated constructors during build/test code generation,
///   within 1,024 typed leaves including the implicit container. `cargo check` does not
///   evaluate them. Open, erased and larger graphs retain runtime cycle validation;
///   scope/config checks remain runtime.
#[macro_export]
macro_rules! compiled_registry {
    ($($tokens:tt)*) => {
        $crate::macros_utils::compiled::registry!($crate::macros_utils::compiled; $($tokens)*)
    };
}

/// The `compiled_async_registry!` macro is used to create an **typed asynchronous dependency registry**
/// with flexible configuration and composition options.
///
/// Requires the `compiled` and `async` features. Use the result with
/// [`async_impl::Container::new`](crate::async_impl::Container::new).
///
/// ### `provide` syntax
///
/// Each `provide` item defines a single asynchronous dependency registration.
/// The following forms are supported:
///
/// ```no_code
/// provide(inst)                             // async instantiator only
/// provide(inst, config = Config::default()) // with configuration
/// provide(inst, finalizer = fin)            // with async finalizer
/// provide(inst, config = Config::default(), finalizer = fin) // with both parameters
/// provide(inst, finalizer = fin, config = Config::default()) // order doesn’t matter
/// ```
///
/// Parameters:
/// - `config` *(optional)* — configuration object.
/// - `finalizer` *(optional)* — asynchronous function called when the dependency is finalized.
///
/// ## Usage patterns
///
/// ### 1. Single `scope`
/// ```rust
/// use froodi::{compiled_async_registry, InstantiateErrorKind, DefaultScope::*};
///
/// async fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// compiled_async_registry! {
///     scope(App) [
///         provide(inst),
///     ]
/// };
/// ```
///
/// ### 2. Multiple `scope`
/// ```rust
/// use froodi::{compiled_async_registry, InstantiateErrorKind, DefaultScope::*};
///
/// async fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// compiled_async_registry! {
///     scope(App) [ provide(inst) ],
///     scope(Session) [ provide(inst) ],
/// };
/// ```
///
/// ### 3. Single `provide`
/// ```rust
/// use froodi::{compiled_async_registry, InstantiateErrorKind, DefaultScope::*};
///
/// async fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// compiled_async_registry! {
///     provide(App, inst)
/// };
/// ```
///
/// ### 4. Multiple `provide`
/// ```rust
/// use froodi::{compiled_async_registry, InstantiateErrorKind, DefaultScope::*};
///
/// async fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// compiled_async_registry! {
///     provide(App, inst),
///     provide(Session, inst),
///     provide(Request, inst),
/// };
/// ```
///
/// ### 5. Combination of one or more `scope` and `provide`
/// ```rust
/// use froodi::{compiled_async_registry, InstantiateErrorKind, DefaultScope::*};
///
/// async fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// compiled_async_registry! {
///     scope(App) [ provide(inst) ],
///     provide(Session, inst),
///     provide(Request, inst),
/// };
/// ```
///
/// ### 6. Using `extend` standalone
/// ```rust
/// use froodi::{compiled_async_registry, DefaultScope::*};
///
/// compiled_async_registry! {
///     extend(compiled_async_registry!())
/// };
/// ```
///
/// ### 7. Using `extend` with different registries
/// ```rust
/// use froodi::{compiled_registry, compiled_async_registry, DefaultScope::*};
///
/// compiled_async_registry! {
///     extend(compiled_async_registry!(), compiled_registry!()),
/// };
/// ```
///
/// ### 8. Using `extend` together with a combination of `scope` and `provide`
/// ```rust
/// use froodi::{compiled_registry, compiled_async_registry, InstantiateErrorKind, DefaultScope::*};
///
/// async fn inst() -> Result<(), InstantiateErrorKind> {
///     Ok(())
/// }
///
/// compiled_async_registry! {
///     scope(App) [ provide(inst) ],
///     provide(Session, inst),
///     extend(compiled_async_registry!(), compiled_registry!()),
/// };
/// ```
///
/// ### 9. Empty macro usage
/// ```rust
/// use froodi::compiled_async_registry;
///
/// let registry = compiled_async_registry!();
/// ```
/// This creates an empty typed fragment; final async construction adds both container registrations.
///
/// ## Compiled-specific behavior
///
/// Typed sync fragments can be extended into this registry. Async instantiators may
/// depend on sync or async providers; typed sync instantiators cannot depend on async providers.
/// `.into_async_registry()` returns [`RegistryWithSync`](crate::async_impl::RegistryWithSync)
/// for native composition without naming the inferred tree type. Final cycle/scope
/// validation is deferred until container construction.
///
/// Provider checks, custom resolver parameters, runtime/context declarations and native-fragment
/// precedence follow [`compiled_registry!`](crate::compiled_registry). `extend(...)` may appear
/// between registrations. The 1,024-leaf cycle-check limit includes both implicit containers.
#[cfg(feature = "async")]
#[macro_export]
macro_rules! compiled_async_registry {
    ($($tokens:tt)*) => {
        $crate::macros_utils::compiled::async_registry!($crate::macros_utils::compiled; $($tokens)*)
    };
}
