use super::{
    frontend::{Collect, CollectedRegistration, IntoFragment},
    linking::{Empty, Link, Provider, RegistryIndex, SyncExecution},
    registration::{NoFinalizer, Provide},
    topology::Topology,
};
use crate::{Config, Registry as RuntimeRegistry, ScopeData, TypeInfo};
use alloc::vec::Vec;
use core::marker::PhantomData;

#[diagnostic::do_not_recommend]
impl<T> Provide<SyncExecution, BoundaryDeps, NoFinalizer> for Boundary<T> {
    type Leaf = BoundaryLeaf<T>;

    fn into_leaf(self, _: ScopeData, _: Config, _: Option<NoFinalizer>) -> Self::Leaf {
        BoundaryLeaf(PhantomData)
    }
}

pub struct RuntimeNode(RuntimeRegistry);

impl IntoFragment for RuntimeRegistry {
    type Tree = RuntimeNode;

    fn into_fragment(self) -> (RuntimeNode, Vec<ScopeData>) {
        let scopes = self.scopes_data.clone();
        (RuntimeNode(self), scopes)
    }
}

impl RegistryIndex for RuntimeNode {
    type Index = Empty;
}

impl<Root> Link<Root, ()> for RuntimeNode {
    type Linked = Self;

    const TOPOLOGY: Topology = Topology::OPEN;

    fn link(self) -> Self {
        self
    }
}

impl Collect for RuntimeNode {
    fn collect(self, _: &mut Vec<CollectedRegistration>, runtime: &mut Vec<RuntimeRegistry>) {
        runtime.push(self.0);
    }
}

pub struct Boundary<T>(PhantomData<fn() -> T>);

/// Declares a provider supplied by a native registry or [`Context`](crate::Context).
///
/// This is an explicit runtime boundary: the provider is available to typed
/// linking, while its value and effective registration are checked at runtime.
pub fn declare<T>() -> Boundary<T> {
    Boundary(PhantomData)
}

pub struct BoundaryDeps;

pub struct BoundaryLeaf<T>(PhantomData<fn() -> T>);

impl<T> RegistryIndex for BoundaryLeaf<T> {
    type Index = Provider<T>;
}

impl<Root, T> Link<Root, ()> for BoundaryLeaf<T> {
    type Linked = Self;

    const TOPOLOGY: Topology = Topology::OPEN;

    fn link(self) -> Self {
        self
    }
}

impl<T: 'static> Collect for BoundaryLeaf<T> {
    fn collect(self, entries: &mut Vec<CollectedRegistration>, _: &mut Vec<RuntimeRegistry>) {
        entries.push(CollectedRegistration {
            key: TypeInfo::of::<T>(),
            data: None,
        });
    }
}
