//! Stage boundaries for `tools/compile_bench.rs`; not part of the container API.

use crate::{
    container::{Linked, ProviderIndex},
    graph::{CollectExecutors, CollectRegistrations, ContainerLeaf, Link, Node, RegistryIndex},
    Registry,
};

fn root<Tree>(registry: Registry<Tree>) -> Node<Tree, ContainerLeaf> {
    Node(registry.tree, ContainerLeaf { scope: registry.scopes[0] })
}

pub fn linking<Tree, Links>(registry: Registry<Tree>)
where
    Node<Tree, ContainerLeaf>: RegistryIndex + Link<ProviderIndex<Tree>, Links>,
{
    core::hint::black_box(root(registry).link());
}

pub fn validated<Tree, Links>(registry: Registry<Tree>)
where
    Node<Tree, ContainerLeaf>: RegistryIndex + Link<ProviderIndex<Tree>, Links>,
{
    let () = <Node<Tree, ContainerLeaf> as Link<ProviderIndex<Tree>, Links>>::VALIDATE;
    core::hint::black_box(root(registry).link());
}

pub fn metadata<Tree, Links>(registry: Registry<Tree>)
where
    Node<Tree, ContainerLeaf>: RegistryIndex + Link<ProviderIndex<Tree>, Links>,
    Linked<Tree, Links>: CollectRegistrations,
{
    let mut linked = root(registry).link();
    let mut metadata = alloc::vec::Vec::new();
    linked.collect_registrations(&mut metadata);
    core::hint::black_box(metadata);
}

pub fn executors<Tree, Links>(registry: Registry<Tree>)
where
    Node<Tree, ContainerLeaf>: RegistryIndex + Link<ProviderIndex<Tree>, Links>,
    Linked<Tree, Links>: CollectExecutors,
{
    let linked = alloc::boxed::Box::new(root(registry).link());
    let mut executors = alloc::vec::Vec::new();
    linked.collect_executors(&mut executors);
    core::hint::black_box(executors);
}
