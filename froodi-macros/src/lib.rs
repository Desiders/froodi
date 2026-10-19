#![no_std]

extern crate alloc;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::parse::Parse;

mod attr_parsing;
mod fragment;
mod injectable;
mod provide;
mod registry;

/// Registers one constructor from an inherent `impl` through `froodi-auto`.
///
/// Put `#[injectable]` on the `impl` and `#[provide(...)]` on one associated
/// constructor. Its parameters are Froodi dependency resolvers, such as
/// `Inject<T>`. The
/// constructor must return `Result<Self, InstantiateErrorKind>` and cannot
/// take `self`. Both synchronous and asynchronous constructors are supported.
///
/// ```rust,ignore
/// use froodi::{DefaultScope::Request, Inject, InstantiatorResult};
/// use froodi_auto::injectable;
/// use std::sync::Arc;
///
/// struct UserRepository {
///     db: Arc<Database>,
/// }
///
/// #[injectable]
/// impl UserRepository {
///     #[provide(Request)]
///     fn new(Inject(db): Inject<Database>) -> InstantiatorResult<Self> {
///         Ok(Self { db })
///     }
/// }
/// # struct Database;
/// ```
///
/// The method attribute accepts these forms; `config` and `finalizer` can
/// appear in either order:
///
/// ```text
/// #[provide(scope)]
/// #[provide(scope, config = expression)]
/// #[provide(scope, finalizer = expression)]
/// #[provide(scope, config = expression, finalizer = expression)]
/// #[provide(scope, finalizer = expression, config = expression)]
/// ```
///
/// `config` is a Froodi `Config`; its `cache_provides` field controls whether
/// the constructed value is reused. `finalizer` is a function or closure called
/// with the constructed value when its scope closes. The finalizer is
/// asynchronous when the constructor is asynchronous.
///
/// `froodi-auto` collects these registrations when its auto-registry method is
/// called. For explicit registration, use [`registry`] instead.
#[proc_macro_attribute]
pub fn injectable(_attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_with(item, injectable::expand)
}

/// Builds a struct from dependencies when `provide::<Type>()` registers it.
///
/// Derive `Construct` on a named, tuple, or unit struct. By default, a field
/// uses `Inject` and must be a shared pointer (`Arc<T>` in thread-safe mode,
/// `Rc<T>` in local mode). `Inject` follows the dependency provider's scope
/// and cache setting. Mark a field with `#[di(inject_transient)]` to construct
/// a fresh value of its exact field type. `#[di(inject)]` explicitly selects
/// the default behavior.
///
/// This follows the `construct_registration` example:
///
/// ```rust
/// use froodi::{instance, registry, Config as ProviderConfig, Container, DefaultScope::App};
/// use std::sync::Arc;
///
/// #[derive(Clone)]
/// struct Config(&'static str);
/// struct RequestId(usize);
///
/// #[derive(froodi::Construct)]
/// struct Service {
///     #[di(inject)]
///     config: Arc<Config>,
///     #[di(inject_transient)]
///     request_id: RequestId,
/// }
///
/// let container = Container::new(registry! {
///     scope(App) [
///         provide(instance(Config("Hello"))),
///         provide(|| Ok(RequestId(7))),
///         provide::<Service>(
///             config = ProviderConfig { cache_provides: false },
///             finalizer = |service: Arc<Service>| println!("Closing {}", service.config.0),
///         ),
///     ],
/// });
/// let first = container.get::<Service>().unwrap();
/// let second = container.get::<Service>().unwrap();
/// assert_eq!(first.config.0, "Hello");
/// assert_eq!(first.request_id.0, 7);
/// assert!(!Arc::ptr_eq(&first, &second));
/// container.close();
/// ```
///
/// Deriving the trait does not register the struct. Add `provide::<Type>()`
/// inside a `registry!` scope block. Its options have these forms:
///
/// ```text
/// provide::<Type>()
/// provide::<Type>(config = ProviderConfig { cache_provides: false })
/// provide::<Type>(finalizer = cleanup)
/// provide::<Type>(config = configuration, finalizer = cleanup)
/// provide::<Type>(finalizer = cleanup, config = configuration)
/// ```
///
/// `config` controls caching of the constructed `Type`; its dependencies keep
/// their own cache settings. Constructed values are cached by default.
/// `finalizer` receives an `Arc<Type>` in thread-safe mode or an `Rc<Type>`
/// in local mode when the scope closes. A derived `Construct` makes a
/// synchronous factory, so a supplied finalizer must also be synchronous.
#[proc_macro_derive(Construct, attributes(di))]
pub fn derive_construct(input: TokenStream) -> TokenStream {
    expand_with(input, provide::expand)
}

/// Declares providers and composes them into a registry.
///
/// Pass the result to `froodi::Container` or `froodi::async_impl::Container`.
/// Use `provide(scope, factory)` for one registration, or group registrations
/// under `scope(scope) [ ... ]`. A factory can be a named function, closure,
/// captured closure, or `instance(value)`. Synchronous and asynchronous
/// factories can appear in the same registry; their Rust types select how
/// they execute.
///
/// This uses the same configuration and request pattern as the registration
/// examples:
///
/// ```rust
/// use froodi::{instance, registry, Container, DefaultScope::{App, Request}, Inject, InstantiateErrorKind};
///
/// #[derive(Clone)]
/// struct Config(&'static str);
///
/// fn make_message(Inject(config): Inject<Config>) -> Result<String, InstantiateErrorKind> {
///     Ok(format!("Hello, {}!", config.0))
/// }
///
/// let configuration = registry! {
///     provide(App, instance(Config("froodi"))),
/// };
/// let app = Container::new(registry! {
///     scope(Request) [
///         provide(make_message),
///     ],
///     extend(configuration),
/// });
/// let request = app.clone().enter_build().unwrap();
/// assert_eq!(&**request.get::<String>().unwrap(), "Hello, froodi!");
/// ```
///
/// # Supported syntax
///
/// At the top level, a `provide` clause includes its scope:
///
/// ```text
/// provide(scope, factory)
/// provide(scope, factory, config = configuration)
/// provide(scope, factory, finalizer = cleanup)
/// provide(scope, factory, config = configuration, finalizer = cleanup)
/// provide(scope, factory, finalizer = cleanup, config = configuration)
/// ```
///
/// A scope block applies one scope to every entry. `provide::<Type>()`
/// registers a type with `#[derive(Construct)]`. Both entry kinds accept the
/// same `config` and `finalizer` options shown above:
///
/// ```text
/// scope(scope) [
///     provide(factory),
///     provide(factory, config = configuration),
///     provide(factory, finalizer = cleanup),
///     provide(factory, config = configuration, finalizer = cleanup),
///     provide(factory, finalizer = cleanup, config = configuration),
///     provide::<Type>(),
///     provide::<Type>(config = configuration),
///     provide::<Type>(finalizer = cleanup),
///     provide::<Type>(config = configuration, finalizer = cleanup),
///     provide::<Type>(finalizer = cleanup, config = configuration),
/// ]
/// ```
///
/// Compose registries or include syntax fragments with these clauses:
///
/// ```text
/// extend(registry_value)
/// extend(first_registry, second_registry)
/// extend_fragment(template!(argument))
/// extend_fragment(first!(), second!())
/// ```
///
/// `registry! {}` creates an empty registry. Leading `use` statements can
/// bring names into a registry body. Clauses and options are evaluated once
/// in source order.
///
/// `config` is a Froodi `Config`. Set `cache_provides: false` when a provider
/// should make a new value on each resolution; it does not change how that
/// provider's dependencies are cached. A `finalizer` receives the produced
/// value when its scope closes. Use a synchronous finalizer with a synchronous
/// factory, or an asynchronous finalizer with an asynchronous factory.
///
/// `Inject<T>` and `InjectTransient<T>` link to registered providers.
/// `InjectCustom<T>` is available for custom runtime resolution. Missing or
/// ambiguous static providers are compilation errors. Closed graphs can also
/// reject cycles and incompatible static scopes at build time; dynamic scopes
/// and erased registries keep runtime validation. Call `.into_registry()` when
/// a first-class erased `Registry` value is needed. A synchronous `Container`
/// cannot resolve an async-only provider; an async container can use both.
#[proc_macro]
pub fn registry(input: TokenStream) -> TokenStream {
    expand_with(input, registry::expand)
}

/// Saves a `registry!` body as a reusable template.
///
/// Declare a fragment at module scope, then include it with `extend_fragment`
/// in another `registry!`. The body uses ordinary registry syntax and is
/// linked together with the surrounding registrations. A fragment does not
/// create a registry value or register anything until it is included.
///
/// This is the pattern used by the `fragments` example:
///
/// ```rust
/// use froodi::{instance, registry, Container, DefaultScope::App, Inject, InstantiateErrorKind};
///
/// #[derive(Clone)]
/// struct Config(u32);
/// struct Service(u32);
///
/// #[froodi::fragment(configuration(config))]
/// registry! {
///     provide(App, instance(config)),
/// }
///
/// # fn main() {
/// let container = Container::new(registry! {
///     extend_fragment(configuration!(Config(7))),
///     provide(App, |Inject(config): Inject<Config>| Ok::<_, InstantiateErrorKind>(Service(config.0))),
/// });
/// assert_eq!(container.get::<Service>().unwrap().0, 7);
/// # }
/// ```
///
/// # Declaration and inclusion
///
/// ```text
/// #[froodi::fragment(name)]
/// registry! { ... }
///
/// #[froodi::fragment(name(arg1, arg2))]
/// registry! { ... }
///
/// #[froodi::fragment(pub(crate) name)]
/// registry! { ... }
///
/// #[froodi::fragment(pub(crate) name(arg1))]
/// registry! { ... }
///
/// #[froodi::fragment(pub name)]
/// registry! { ... }
///
/// #[froodi::fragment(pub name(arg1))]
/// registry! { ... }
/// ```
///
/// Include one or more fragments in a registry:
///
/// ```text
/// registry! {
///     extend_fragment(name!()),
///     extend_fragment(other!(value1, value2), another!()),
/// }
/// ```
///
/// Parameters are untyped names. Each supplied expression is evaluated once,
/// in order, at the inclusion point; ordinary Rust rules handle moves and
/// borrows. Types are inferred when the final registry is compiled. The body
/// may use `provide`, `provide::<Type>()`, `scope`, `extend`, local `use`
/// statements, and nested `extend_fragment` calls. Recursive inclusion is
/// rejected. Use `extend(...)` for registry *values* and `.into_registry()`
/// when the final composition must become an erased `Registry` value.
///
/// An exported fragment can be imported or renamed with `use` like another
/// macro. `crate::` paths in its body refer to the crate that declared it;
/// unqualified names resolve where it is included. Fragment declarations do
/// not have typed parameters, generics, or lifetime parameters.
#[proc_macro_attribute]
pub fn fragment(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_with(attr, |fragment| fragment::expand(fragment, item.into()))
}

fn expand_with<F, I, K>(input: TokenStream, f: F) -> TokenStream
where
    F: FnOnce(I) -> syn::Result<K>,
    I: Parse,
    K: ToTokens,
{
    expand(syn::parse(input).and_then(f))
}

fn expand<T>(result: syn::Result<T>) -> TokenStream
where
    T: ToTokens,
{
    match result {
        Ok(tokens) => (quote! { #tokens }).into(),
        Err(err) => err.into_compile_error().into(),
    }
}
