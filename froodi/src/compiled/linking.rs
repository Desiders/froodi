//! Container-independent provider witnesses and tree traversal.

use super::{dependency_resolver::RuntimeDependency, topology::Topology};
use crate::{Inject, InjectTransient};
use core::marker::PhantomData;

pub struct Here;

pub struct L<Path>(PhantomData<Path>);

pub struct R<Path>(PhantomData<Path>);

pub struct Empty;

unsafe impl Size for Empty {
    const SIZE: usize = 0;
}

unsafe impl<Root> Link<Root, ()> for Empty {
    type Linked = Empty;

    const TOPOLOGY: Topology = Topology::EMPTY;

    #[inline]
    fn link(self) -> Empty {
        self
    }
}

pub struct Node<Left, Right>(pub Left, pub Right);

/// # Safety
/// `SIZE` must equal the number of indexed provider leaves in declaration order.
pub unsafe trait Size {
    const SIZE: usize;
}

unsafe impl<Left: Size, Right: Size> Size for Node<Left, Right> {
    const SIZE: usize = Left::SIZE + Right::SIZE;
}

/// Omits instantiators, dependency tuples and finalizers from provider lookup.
///
/// # Safety
/// The index must preserve the value tree's provider types, execution kinds and leaf order.
pub unsafe trait RegistryIndex {
    type Index;
}

pub struct Provider<Out>(PhantomData<fn() -> Out>);

unsafe impl<Out> Size for Provider<Out> {
    const SIZE: usize = 1;
}

unsafe impl<Out> ProviderPath<Out, Here> for Provider<Out> {
    type Provider = Self;

    const INDEX: usize = 0;
}

impl<Out, Execution> SupportsExecution<Execution> for Provider<Out> {}

unsafe impl<Left: RegistryIndex, Right: RegistryIndex> RegistryIndex for Node<Left, Right> {
    type Index = Node<Left::Index, Right::Index>;
}

unsafe impl RegistryIndex for Empty {
    type Index = Empty;
}

/// rustc infers `Path`; missing or ambiguous providers fail trait resolution.
///
/// # Safety
/// `INDEX` must select the provider of exactly `T` at `Path` in this index.
#[diagnostic::on_unimplemented(
    message = "no registration provides `{T}`",
    label = "`{T}` is requested here, but no `provide(...)` in the registry produces it",
    note = "register an instantiator or `instance(...)` that returns `{T}`, or `extend(...)` a registry that does"
)]
pub unsafe trait ProviderPath<T, Path> {
    type Provider;

    const INDEX: usize;
}

unsafe impl<T, Path, Left: ProviderPath<T, Path>, Right> ProviderPath<T, L<Path>> for Node<Left, Right> {
    type Provider = Left::Provider;

    const INDEX: usize = Left::INDEX;
}

unsafe impl<T, Path, Left: Size, Right: ProviderPath<T, Path>> ProviderPath<T, R<Path>> for Node<Left, Right> {
    type Provider = Right::Provider;

    const INDEX: usize = Left::SIZE + Right::INDEX;
}

pub struct LinkedInject<Path>(PhantomData<Path>);

pub struct LinkedInjectTransient<Path>(PhantomData<Path>);

pub struct ByResolver;

pub struct SyncExecution;

#[cfg(feature = "async")]
pub struct AsyncExecution;

#[diagnostic::on_unimplemented(
    message = "this registration cannot be constructed synchronously",
    label = "a sync instantiator depends on it",
    note = "a sync instantiator may not depend on an async registration; make the dependent instantiator async"
)]
pub trait SupportsExecution<Execution> {}

// Check execution after provider inference, so an async provider is not reported as missing.
macro_rules! impl_supports_execution {
    ([$($provider:ident),*]) => {
        impl<Execution, $($provider: SupportsExecution<Execution>,)*> SupportsExecution<Execution> for ($($provider,)*) {}
    };
}

all_the_tuples!(impl_supports_execution);

/// `Links` mirrors the tree shape and is inferred by rustc.
///
/// # Safety
/// Linked values, topology targets and provider index positions must describe the same
/// registrations. Implementations must not forge dependency types or reorder registrations.
pub unsafe trait Link<Root, Links> {
    type Linked;

    const TOPOLOGY: Topology;
    const VALIDATE: () = Self::TOPOLOGY.validate();

    fn link(self) -> Self::Linked;
}

unsafe impl<Root, Left: Link<Root, LLinks>, Right: Link<Root, RLinks>, LLinks, RLinks> Link<Root, (LLinks, RLinks)> for Node<Left, Right> {
    type Linked = Node<Left::Linked, Right::Linked>;

    const TOPOLOGY: Topology = Topology::branch(&Left::TOPOLOGY, &Right::TOPOLOGY);

    #[inline]
    fn link(self) -> Self::Linked {
        Node(self.0.link(), self.1.link())
    }
}

#[diagnostic::on_unimplemented(
    message = "no static registration provides the instantiator parameter `{Self}`",
    note = "custom resolvers in typed registrations use RuntimeDependency<T>; ordinary runtime fragments accept them directly"
)]
pub trait LinkDependency<Root, Path> {
    type Provider;

    const TARGET: Option<usize>;
}

impl<Root: ProviderPath<T, Path>, T, Path> LinkDependency<Root, LinkedInject<Path>> for Inject<T> {
    type Provider = Root::Provider;

    const TARGET: Option<usize> = Some(Root::INDEX);
}

impl<Root: ProviderPath<T, Path>, T, Path> LinkDependency<Root, LinkedInjectTransient<Path>> for InjectTransient<T> {
    type Provider = Root::Provider;

    const TARGET: Option<usize> = Some(Root::INDEX);
}

impl<Root, T> LinkDependency<Root, ByResolver> for RuntimeDependency<T> {
    type Provider = ();

    const TARGET: Option<usize> = None;
}

pub trait LinkDependencies<Root, Links> {
    type Providers;

    const TARGETS: &'static [Option<usize>];
}

macro_rules! link_deps {
    ([$($dep:ident $path:ident),*]) => {
        impl<Root, $($dep: LinkDependency<Root, $path>, $path,)*> LinkDependencies<Root, ($($path,)*)> for ($($dep,)*) {
            type Providers = ($($dep::Provider,)*);

            const TARGETS: &'static [Option<usize>] = &[$($dep::TARGET,)*];
        }
    };
}

all_the_tuple_pairs!(link_deps);
