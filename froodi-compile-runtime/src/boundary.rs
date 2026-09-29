//! Registrations that declare where a static graph meets runtime values.
//!
//! `provide(scope, context::<T>())` states that `T` arrives through the `Context` of a container
//! in that scope. Static factories may depend on it; a container whose context lacks it fails to
//! resolve it with `ResolveErrorKind::NoContextValue`.
//!
//! `provide(scope, runtime::<T>())` states that `T` is provided at runtime, by a runtime registry
//! the container is built with. Static factories may then depend on `T`: rustc links them to the
//! boundary, and the graph compiler links the boundary to the real provider when the container is
//! built. Without such a declaration a static dependency on a runtime-only type does not compile.

use alloc::vec::Vec;
use core::{any::TypeId, marker::PhantomData};

use froodi_compile_core::{DependencyRequest, ExecutionKind, Origin, Registration, RequestMode, Target, ValueSource};

use crate::{
    config::Config,
    container::Container,
    errors::{ResolveErrorKind, TypeInfo},
    finalizer::{MaybeFinalizer, NoFinalizer},
    graph::{At, CollectRuntime, Describe, Entry, Exec, Finalize, Has, Here, Link, Meta, Reg, Size, Walk},
    instantiator::Instantiator,
    scope::ScopeData,
    thread_safety::{RcAnyThreadSafety, SendSafety, SyncSafety},
};

/// What `provide(...)` accepts: a factory, or a boundary declaration.
pub trait Provide<D, Fin> {
    type Leaf;

    #[doc(hidden)]
    fn into_leaf(self, finalizer: Fin, meta: Meta) -> Self::Leaf;
}

impl<F: Instantiator<D>, D, Fin: MaybeFinalizer<F::Provides>> Provide<D, Fin> for F {
    type Leaf = Reg<F::Provides, F, D, Fin>;

    #[inline]
    fn into_leaf(self, finalizer: Fin, meta: Meta) -> Self::Leaf {
        Reg::new(self, finalizer, meta)
    }
}

/// `runtime::<T>()`: see the module docs.
pub struct RuntimeBoundary<T>(PhantomData<fn() -> T>);

/// Declares that `T` is provided at runtime. See the module docs.
#[inline]
#[must_use]
pub const fn runtime<T>() -> RuntimeBoundary<T> {
    RuntimeBoundary(PhantomData)
}

#[doc(hidden)]
pub struct BoundaryDeps;

impl<T> Provide<BoundaryDeps, NoFinalizer> for RuntimeBoundary<T> {
    type Leaf = ImportLeaf<T>;

    #[inline]
    fn into_leaf(self, _finalizer: NoFinalizer, meta: Meta) -> Self::Leaf {
        ImportLeaf {
            scope: meta.scope,
            origin: meta.origin,
            config: meta.config,
            marker: PhantomData,
        }
    }
}

/// The leaf of a `runtime::<T>()` declaration. Its value is the real provider's value.
pub struct ImportLeaf<T> {
    scope: ScopeData,
    origin: Origin,
    config: Config,
    marker: PhantomData<fn() -> T>,
}

impl<T> Size for ImportLeaf<T> {
    const SIZE: usize = 1;
}

impl<T> Has<T, Here> for ImportLeaf<T> {}

impl<T> At<Here> for ImportLeaf<T> {
    type Item = Self;
    const INDEX: usize = 0;

    #[inline]
    fn at(&self) -> &Self {
        self
    }
}

impl<Root, T> Link<Root, ()> for ImportLeaf<T> {
    type Linked = Self;

    #[inline]
    fn link(self) -> Self {
        self
    }
}

impl<T> CollectRuntime for ImportLeaf<T> {}

fn provider(container: &Container, index: usize) -> usize {
    container.edges(index)[0].target.index()
}

impl<Root, T: 'static> Exec<Root> for ImportLeaf<T> {
    type Provides = T;

    fn construct(&self, _root: &Root, container: &Container, index: usize) -> Result<T, ResolveErrorKind> {
        container.transient_at::<T>(provider(container, index))
    }

    fn construct_shared(&self, _root: &Root, container: &Container, index: usize) -> Result<RcAnyThreadSafety, ResolveErrorKind>
    where
        T: SendSafety + SyncSafety,
    {
        container.shared(provider(container, index))
    }
}

impl<T> Finalize for ImportLeaf<T> {
    unsafe fn finalize(&self, _value: RcAnyThreadSafety) {}
}

impl<Root, T: SendSafety + SyncSafety + 'static> Walk<Root> for ImportLeaf<T> {
    #[allow(private_interfaces)]
    fn walk(&self, entries: &mut Vec<Entry>) {
        entries.push(Entry::of::<Root, Self>(self));
    }
}

impl<T: 'static> Describe for ImportLeaf<T> {
    fn describe(&self, out: &mut Vec<Registration<TypeId>>) {
        out.push(Registration {
            key: TypeId::of::<T>(),
            type_name: core::any::type_name::<T>(),
            requests: alloc::vec![DependencyRequest {
                target: Target::Key(TypeId::of::<T>()),
                mode: RequestMode::Shared,
                type_name: core::any::type_name::<T>(),
            }],
            scope: self.scope.into(),
            cache_provides: self.config.cache_provides,
            finalizer: None,
            execution: ExecutionKind::Sync,
            source: ValueSource::Runtime,
            replaces: false,
            origin: Some(self.origin),
        });
    }
}

/// `context::<T>()`: see the module docs.
pub struct ContextBoundary<T>(PhantomData<fn() -> T>);

/// Declares that `T` arrives through `Context`. See the module docs.
#[inline]
#[must_use]
pub const fn context<T>() -> ContextBoundary<T> {
    ContextBoundary(PhantomData)
}

impl<T> Provide<BoundaryDeps, NoFinalizer> for ContextBoundary<T> {
    type Leaf = ContextLeaf<T>;

    #[inline]
    fn into_leaf(self, _finalizer: NoFinalizer, meta: Meta) -> Self::Leaf {
        ContextLeaf {
            scope: meta.scope,
            origin: meta.origin,
            marker: PhantomData,
        }
    }
}

/// The leaf of a `context::<T>()` declaration. A context value of `T` fills its cache slot when
/// a container is created, so the leaf itself only runs when the value is missing.
pub struct ContextLeaf<T> {
    scope: ScopeData,
    origin: Origin,
    marker: PhantomData<fn() -> T>,
}

impl<T> Size for ContextLeaf<T> {
    const SIZE: usize = 1;
}

impl<T> Has<T, Here> for ContextLeaf<T> {}

impl<T> At<Here> for ContextLeaf<T> {
    type Item = Self;
    const INDEX: usize = 0;

    #[inline]
    fn at(&self) -> &Self {
        self
    }
}

impl<Root, T> Link<Root, ()> for ContextLeaf<T> {
    type Linked = Self;

    #[inline]
    fn link(self) -> Self {
        self
    }
}

impl<T> CollectRuntime for ContextLeaf<T> {}

impl<Root, T: 'static> Exec<Root> for ContextLeaf<T> {
    type Provides = T;

    fn construct(&self, _root: &Root, container: &Container, _index: usize) -> Result<T, ResolveErrorKind> {
        Err(ResolveErrorKind::NoContextValue {
            type_info: TypeInfo::of::<T>(),
            scope: container.scope().name,
        })
    }
}

impl<T> Finalize for ContextLeaf<T> {
    unsafe fn finalize(&self, _value: RcAnyThreadSafety) {}
}

impl<Root, T: SendSafety + SyncSafety + 'static> Walk<Root> for ContextLeaf<T> {
    #[allow(private_interfaces)]
    fn walk(&self, entries: &mut Vec<Entry>) {
        entries.push(Entry::of::<Root, Self>(self));
    }
}

impl<T: 'static> Describe for ContextLeaf<T> {
    fn describe(&self, out: &mut Vec<Registration<TypeId>>) {
        out.push(Registration {
            key: TypeId::of::<T>(),
            type_name: core::any::type_name::<T>(),
            requests: Vec::new(),
            scope: self.scope.into(),
            cache_provides: true,
            finalizer: None,
            execution: ExecutionKind::Sync,
            source: ValueSource::Context,
            replaces: false,
            origin: Some(self.origin),
        });
    }
}
