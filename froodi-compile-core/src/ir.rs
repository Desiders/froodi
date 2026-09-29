//! Registration IR: what a frontend knows about a registry before the graph is compiled.
//!
//! The IR models Froodi registrations and dependency requests, not a plain `T -> U` type graph.
//! A registration keeps its scope, cache policy, finalizer, execution kind, value source and
//! source origin; a dependency request keeps its mode, so `Inject<T>` and `InjectTransient<T>`
//! stay distinct edges.
//!
//! The graph is generic over a binding key `K`. The compiler only compares keys for equality and
//! order; it never interprets them. A runtime frontend passes `TypeId`, a test passes `&str`.
//! Where rustc already proved which registration a request targets, the frontend passes
//! [`Target::Id`] and no key comparison happens at all.

use alloc::vec::Vec;
use core::fmt::{self, Display, Formatter};

/// Position of a registration in its graph. Frontends assign ids in declaration order, so the
/// same registry source always yields the same ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegistrationId(pub u32);

impl RegistrationId {
    #[inline]
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// How a dependency is requested. This is the semantic difference between the Froodi parameter
/// wrappers, preserved as edge metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequestMode {
    /// `Inject<T>`: `get`-like. Scoped, shared, subject to the target's cache policy and finalizer.
    Shared,
    /// `InjectTransient<T>`: `get_transient`-like. A fresh value, the provided-value cache is not used.
    Transient,
    /// A custom `DependencyResolver`: user code that reads the container at runtime. The request
    /// is recorded, but its target is opaque to the compiler, so it is never resolved to an edge
    /// and never reported as missing.
    Resolver,
}

/// What a dependency request points at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target<K> {
    /// rustc already resolved the request to this registration (typed frontend).
    Id(RegistrationId),
    /// The compiler resolves the request by binding key (runtime/indexed frontend).
    Key(K),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyRequest<K> {
    pub target: Target<K>,
    pub mode: RequestMode,
    /// Display name of the requested type, used only in diagnostics.
    pub type_name: &'static str,
}

/// A scope as the compiler sees it: the three values of Froodi's `ScopeData`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeKey {
    pub priority: u8,
    pub name: &'static str,
    pub skipped_by_default: bool,
}

impl Display for ScopeKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}, priority {}", self.name, self.priority)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExecutionKind {
    Sync,
    Async,
}

/// Where the provided value comes from at runtime. The graph structure is static in every case;
/// only the value is produced differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueSource {
    /// A function or closure called with its resolved dependencies.
    Factory,
    /// `instance(value)`: a runtime value, no dependencies.
    Instance,
    /// The value is expected in the `Context` of the owning scope.
    Context,
    /// The value is provided by a runtime registry attached to the container.
    Runtime,
    /// The container itself.
    Container,
}

/// Where a registration was written, for diagnostics. It is never used as type identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Origin {
    /// The instantiator expression as written, e.g. `make_repository`.
    pub expr: &'static str,
    pub file: &'static str,
    pub line: u32,
    pub column: u32,
}

impl Display for Origin {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` at {}:{}:{}", self.expr, self.file, self.line, self.column)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registration<K> {
    /// Binding key of the provided type.
    pub key: K,
    /// Display name of the provided type, used only in diagnostics.
    pub type_name: &'static str,
    pub requests: Vec<DependencyRequest<K>>,
    pub scope: ScopeKey,
    /// `Config::cache_provides`. Independent of the scope.
    pub cache_provides: bool,
    /// Execution kind of the finalizer, if the registration has one.
    pub finalizer: Option<ExecutionKind>,
    pub execution: ExecutionKind,
    pub source: ValueSource,
    pub origin: Option<Origin>,
}

/// A registry before compilation: registrations in declaration order plus the scope hierarchy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Graph<K> {
    pub registrations: Vec<Registration<K>>,
    /// Every scope of the hierarchy (`Scopes::all()`), in any order.
    pub scopes: Vec<ScopeKey>,
}

impl<K> Graph<K> {
    #[inline]
    #[must_use]
    pub const fn new(scopes: Vec<ScopeKey>) -> Self {
        Self {
            registrations: Vec::new(),
            scopes,
        }
    }

    /// Appends a registration and returns its id.
    ///
    /// # Panics
    /// Panics if the graph already holds `u32::MAX` registrations.
    pub fn push(&mut self, registration: Registration<K>) -> RegistrationId {
        let id = RegistrationId(u32::try_from(self.registrations.len()).expect("too many registrations"));
        self.registrations.push(registration);
        id
    }
}
