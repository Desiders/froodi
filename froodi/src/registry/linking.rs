//! Container-independent provider witnesses and tree traversal.

use super::topology::Topology;
use crate::inject::InjectCustom;
use crate::{Inject, InjectTransient};
use core::marker::PhantomData;

pub struct Here;

pub struct L<Path>(PhantomData<Path>);

pub struct R<Path>(PhantomData<Path>);

pub struct Empty;

impl Size for Empty {
    const SIZE: usize = 0;
}

impl<Root> Link<Root, ()> for Empty {
    type Linked = Empty;

    const TOPOLOGY: Topology = Topology::EMPTY;

    #[inline]
    fn link(self) -> Empty {
        self
    }
}

pub struct Node<Left, Right>(pub Left, pub Right);

/// Counts declaration leaves before replacement assembly.
pub trait Size {
    const SIZE: usize;
}

impl<Left: Size, Right: Size> Size for Node<Left, Right> {
    const SIZE: usize = Left::SIZE + Right::SIZE;
}

/// Omits instantiators, dependency tuples and finalizers from provider lookup.
pub trait RegistryIndex {
    type Index;
}

pub struct Provider<Out>(PhantomData<fn() -> Out>);

impl<Out> Size for Provider<Out> {
    const SIZE: usize = 1;
}

impl<Out> ProviderPath<Out, Here> for Provider<Out> {
    type Provider = Self;

    const INDEX: usize = 0;
}

impl<Out, Execution, Request> SupportsExecution<Execution, Request> for Provider<Out> {}

impl<Left: RegistryIndex, Right: RegistryIndex> RegistryIndex for Node<Left, Right> {
    type Index = Node<Left::Index, Right::Index>;
}

impl RegistryIndex for Empty {
    type Index = Empty;
}

/// rustc infers `Path`; missing or ambiguous providers fail trait resolution.
#[diagnostic::on_unimplemented(
    message = "no registration provides `{T}`",
    label = "`{T}` is requested here, but no `provide(...)` in the registry produces it",
    note = "register an instantiator or `instance(...)` that returns `{T}`, or `extend(...)` a registry that does"
)]
pub trait ProviderPath<T, Path> {
    type Provider;

    const INDEX: usize;
}

impl<T, Path, Left: ProviderPath<T, Path>, Right> ProviderPath<T, L<Path>> for Node<Left, Right> {
    type Provider = Left::Provider;

    const INDEX: usize = Left::INDEX;
}

impl<T, Path, Left: Size, Right: ProviderPath<T, Path>> ProviderPath<T, R<Path>> for Node<Left, Right> {
    type Provider = Right::Provider;

    const INDEX: usize = Left::SIZE + Right::INDEX;
}

pub struct LinkedInject<Path>(PhantomData<Path>);

pub struct LinkedInjectTransient<Path>(PhantomData<Path>);

pub struct ByInjectCustom;

pub struct SyncExecution;

#[cfg(feature = "async")]
pub struct AsyncExecution;

#[diagnostic::on_unimplemented(
    message = "cannot resolve `{Request}` in a synchronous instantiator: its selected registration is async",
    label = "`{Request}` requires async construction",
    note = "make the dependent instantiator async"
)]
pub trait SupportsExecution<Execution, Request> {}

impl<Execution, T> SupportsExecution<Execution, InjectCustom<T>> for () {}

// Check execution after provider inference, so an async provider is not reported as missing.
macro_rules! impl_supports_execution {
    ([$($provider:ident $request:ident),*]) => {
        impl<Execution, $($provider: SupportsExecution<Execution, $request>, $request,)*>
            SupportsExecution<Execution, ($($request,)*)> for ($($provider,)*) {}
    };
}

all_the_tuple_pairs!(impl_supports_execution);

/// `Links` is inferred alongside the tree; collection preserves its declaration order.
pub trait Link<Root, Links> {
    type Linked;

    const TOPOLOGY: Topology;
    const VALIDATE: () = Self::TOPOLOGY.validate();

    fn link(self) -> Self::Linked;
}

impl<Root, Left: Link<Root, LLinks>, Right: Link<Root, RLinks>, LLinks, RLinks> Link<Root, (LLinks, RLinks)> for Node<Left, Right> {
    type Linked = Node<Left::Linked, Right::Linked>;

    const TOPOLOGY: Topology = Topology::branch(&Left::TOPOLOGY, &Right::TOPOLOGY);

    #[inline]
    fn link(self) -> Self::Linked {
        Node(self.0.link(), self.1.link())
    }
}

#[diagnostic::on_unimplemented(
    message = "unsupported instantiator parameter `{Self}`; use `InjectCustom<{Self}>` for a custom resolver",
    label = "wrap this custom resolver in `InjectCustom<{Self}>`",
    note = "registration parameters use Inject<T>, InjectTransient<T> or InjectCustom<T>"
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

impl<Root, T> LinkDependency<Root, ByInjectCustom> for InjectCustom<T> {
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
