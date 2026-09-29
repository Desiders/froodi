//! `context::<T>()` links static dependencies to scope context values.
//! `runtime::<T>()` links them to providers resolved by the graph compiler at startup.

use alloc::vec::Vec;
use core::{any::TypeId, marker::PhantomData};

use froodi_compile_core::{CompiledEdge, DependencyRequest, ExecutionKind, Origin, Registration, RequestMode, Target, ValueSource};

use crate::{
    config::Config,
    container::Container,
    errors::{ResolveErrorKind, TypeInfo},
    finalizer::{MaybeFinalizer, NoFinalizer},
    graph::{CollectExecutors, CollectRuntime, ConstructRegistration, Describe, Finalize, Link, Meta, Reg, RegistrationExecutor},
    instantiator::Instantiator,
    scope::ScopeData,
    thread_safety::{RcAnyThreadSafety, SendSafety, SyncSafety},
};

/// What `provide(...)` accepts: an instantiator, or a boundary declaration.
pub trait Provide<Deps, Fin> {
    type Leaf;

    #[doc(hidden)]
    fn into_leaf(self, finalizer: Fin, meta: Meta) -> Self::Leaf;
}

impl<Inst: Instantiator<Deps>, Deps, Fin: MaybeFinalizer<Inst::Provides>> Provide<Deps, Fin> for Inst {
    type Leaf = Reg<Inst::Provides, Inst, Deps, Fin>;

    #[inline]
    fn into_leaf(self, finalizer: Fin, meta: Meta) -> Self::Leaf {
        Reg::new(self, finalizer, meta)
    }
}

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

/// Forwards `Inject` to the provider's allocation, preserving its cache and finalizer policy.
pub struct ImportLeaf<T> {
    scope: ScopeData,
    origin: Origin,
    config: Config,
    marker: PhantomData<fn() -> T>,
}

impl<Root, T> Link<Root, ()> for ImportLeaf<T> {
    type Linked = Self;

    #[inline]
    fn link(self) -> Self {
        self
    }
}

impl<T> CollectRuntime for ImportLeaf<T> {}

impl<T: 'static> ConstructRegistration for ImportLeaf<T> {
    type Provides = T;

    fn construct(&self, container: &Container, edges: &[CompiledEdge]) -> Result<T, ResolveErrorKind> {
        container.get_transient_at::<T>(edges[0].target.index())
    }

    fn construct_inject(&self, container: &Container, edges: &[CompiledEdge]) -> Result<RcAnyThreadSafety, ResolveErrorKind>
    where
        T: SendSafety + SyncSafety,
    {
        container.get_at(edges[0].target.index())
    }
}

impl<T> Finalize for ImportLeaf<T> {
    unsafe fn finalize(&self, _value: RcAnyThreadSafety) {}
}

impl<T: SendSafety + SyncSafety + 'static> CollectExecutors for ImportLeaf<T> {
    #[allow(private_interfaces)]
    fn collect_executors(&self, executors: &mut Vec<RegistrationExecutor>) {
        executors.push(RegistrationExecutor::of::<Self>(self));
    }
}

impl<T: 'static> Describe for ImportLeaf<T> {
    fn describe(&self, out: &mut Vec<Registration<TypeId>>) {
        out.push(Registration {
            key: TypeId::of::<T>(),
            type_name: core::any::type_name::<T>(),
            requests: alloc::vec![DependencyRequest {
                target: Target::Key(TypeId::of::<T>()),
                mode: RequestMode::Inject,
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

/// Context fills the cache slot at container creation; this leaf runs only when the value is missing.
pub struct ContextLeaf<T> {
    scope: ScopeData,
    origin: Origin,
    marker: PhantomData<fn() -> T>,
}

impl<Root, T> Link<Root, ()> for ContextLeaf<T> {
    type Linked = Self;

    #[inline]
    fn link(self) -> Self {
        self
    }
}

impl<T> CollectRuntime for ContextLeaf<T> {}

impl<T: 'static> ConstructRegistration for ContextLeaf<T> {
    type Provides = T;

    fn construct(&self, container: &Container, _edges: &[CompiledEdge]) -> Result<T, ResolveErrorKind> {
        Err(ResolveErrorKind::NoContextValue {
            type_info: TypeInfo::of::<T>(),
            scope: container.scope().name,
        })
    }
}

impl<T> Finalize for ContextLeaf<T> {
    unsafe fn finalize(&self, _value: RcAnyThreadSafety) {}
}

impl<T: SendSafety + SyncSafety + 'static> CollectExecutors for ContextLeaf<T> {
    #[allow(private_interfaces)]
    fn collect_executors(&self, executors: &mut Vec<RegistrationExecutor>) {
        executors.push(RegistrationExecutor::of::<Self>(self));
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
