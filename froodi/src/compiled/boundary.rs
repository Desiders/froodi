use super::{
    linking::{Empty, Link, Provider, RegistryIndex},
    registration::{NoFinalizer, Provide},
    registry::{Collect, CollectedRegistration, IntoFragment},
    topology::Topology,
};
use crate::{Config, Registry as RuntimeRegistry, ScopeData, TypeInfo};
use alloc::vec::Vec;
use core::marker::PhantomData;

impl<T, Execution> Provide<Execution, BoundaryDeps, NoFinalizer> for Boundary<T> {
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

pub fn runtime<T>() -> Boundary<T> {
    Boundary(PhantomData)
}

// Boundary declarations only contribute provider witnesses; Froodi Context/cache remains authoritative.
pub fn context<T>() -> Boundary<T> {
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
