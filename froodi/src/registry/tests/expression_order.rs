extern crate std;

use alloc::{string::String, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};

#[cfg(feature = "async")]
use crate::{async_impl::Container as AsyncContainer, async_registry};
use crate::{
    registry, utils::thread_safety::RcThreadSafety, Config, Container, DefaultScope, InstantiateErrorKind, Scope, ScopeData, Scopes,
};

struct CloneProbe(RcThreadSafety<AtomicUsize>);

impl Clone for CloneProbe {
    fn clone(&self) -> Self {
        self.0.fetch_add(1, Ordering::SeqCst);
        Self(self.0.clone())
    }
}

#[derive(Clone)]
struct Value<const ID: usize>(CloneProbe, String);

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct MoveScope(DefaultScope);

impl From<MoveScope> for ScopeData {
    fn from(scope: MoveScope) -> Self {
        scope.0.into()
    }
}

impl Scope for MoveScope {
    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn priority(&self) -> u8 {
        self.0.priority()
    }

    fn is_skipped_by_default(&self) -> bool {
        self.0.is_skipped_by_default()
    }
}

impl Scopes<5> for MoveScope {
    type Scope = Self;

    fn all() -> (Self, [Self; 5]) {
        let (root, children) = DefaultScope::all();
        (Self(root), children.map(Self))
    }
}

macro_rules! sync_instantiator {
    ($value:expr) => {{
        let value = $value;
        move || Ok::<_, InstantiateErrorKind>(value.clone())
    }};
}

macro_rules! sync_finalizer {
    ($type:ty) => {
        |_: RcThreadSafety<$type>| {}
    };
}

#[cfg(feature = "async")]
macro_rules! async_instantiator {
    ($value:expr) => {{
        let value = $value;
        move || {
            let value = value.clone();
            async move { Ok::<_, InstantiateErrorKind>(value) }
        }
    }};
}

#[cfg(feature = "async")]
macro_rules! async_finalizer {
    ($type:ty) => {
        |_: RcThreadSafety<$type>| async {}
    };
}

macro_rules! ordered_registry {
    ($frontend:ident, $instantiator:ident, $finalizer:ident) => {{
        let mut events = Vec::new();
        let clones = RcThreadSafety::new(AtomicUsize::new(0));
        let value = |id| (CloneProbe(clones.clone()), String::from(id));
        let (probe, first) = value("first");
        let (second_probe, second) = value("second");
        let (third_probe, third) = value("third");
        let (fourth_probe, fourth) = value("fourth");
        let (fifth_probe, fifth) = value("fifth");
        let (sixth_probe, sixth) = value("sixth");
        let (seventh_probe, seventh) = value("seventh");
        let first_scope = MoveScope(DefaultScope::App);
        let group_scope = MoveScope(DefaultScope::App);
        let registry = $frontend! {
            provide({
                events.push("scope:first");
                first_scope
            }, {
                events.push("inst:first");
                $instantiator!(Value::<0>(probe, first))
            }, finalizer = {
                events.push("finalizer:first");
                $finalizer!(Value<0>)
            }, config = {
                events.push("config:first");
                Config { cache_provides: false }
            }),
            extend({
                events.push("extend:second");
                $frontend! {
                    provide(MoveScope(DefaultScope::App), $instantiator!(Value::<1>(second_probe, second))),
                }
            }, {
                events.push("extend:third");
                $frontend! {
                    provide(MoveScope(DefaultScope::App), $instantiator!(Value::<2>(third_probe, third))),
                }
            }),
            scope({
                events.push("scope:group");
                group_scope
            }) [
                provide({
                    events.push("inst:fourth");
                    $instantiator!(Value::<3>(fourth_probe, fourth))
                }, config = {
                    events.push("config:fourth");
                    Config::default()
                }, finalizer = {
                    events.push("finalizer:fourth");
                    $finalizer!(Value<3>)
                }),
                provide({
                    events.push("inst:fifth");
                    $instantiator!(Value::<4>(fifth_probe, fifth))
                }),
            ],
            extend({
                events.push("extend:sixth");
                $frontend! {
                    provide(MoveScope(DefaultScope::App), $instantiator!(Value::<5>(sixth_probe, sixth))),
                }
            }),
            provide({
                events.push("scope:seventh");
                MoveScope(DefaultScope::App)
            }, {
                events.push("inst:seventh");
                $instantiator!(Value::<6>(seventh_probe, seventh))
            }),
        };
        assert_eq!(
            events,
            [
                "scope:first",
                "inst:first",
                "finalizer:first",
                "config:first",
                "extend:second",
                "extend:third",
                "scope:group",
                "inst:fourth",
                "config:fourth",
                "finalizer:fourth",
                "inst:fifth",
                "extend:sixth",
                "scope:seventh",
                "inst:seventh",
            ]
        );
        assert_eq!(clones.load(Ordering::SeqCst), 0);
        (registry, clones)
    }};
}

#[test]
fn sync_expressions_are_evaluated_once_in_source_order() {
    let (registry, clones) = ordered_registry!(registry, sync_instantiator, sync_finalizer);
    let container = Container::new(registry);
    assert_eq!(clones.load(Ordering::SeqCst), 0);
    let first = container.get::<Value<0>>().unwrap();
    assert!(RcThreadSafety::ptr_eq(&first.0 .0, &clones));
    assert_eq!(first.1, "first");
    assert_eq!(container.get::<Value<1>>().unwrap().1, "second");
    assert_eq!(container.get::<Value<2>>().unwrap().1, "third");
    assert_eq!(container.get::<Value<3>>().unwrap().1, "fourth");
    assert_eq!(container.get::<Value<4>>().unwrap().1, "fifth");
    assert_eq!(container.get::<Value<5>>().unwrap().1, "sixth");
    assert_eq!(container.get::<Value<6>>().unwrap().1, "seventh");
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_expressions_are_evaluated_once_in_source_order() {
    let (registry, clones) = ordered_registry!(async_registry, async_instantiator, async_finalizer);
    let container = AsyncContainer::new(registry);
    assert_eq!(clones.load(Ordering::SeqCst), 0);
    let first = container.get::<Value<0>>().await.unwrap();
    assert!(RcThreadSafety::ptr_eq(&first.0 .0, &clones));
    assert_eq!(first.1, "first");
    assert_eq!(container.get::<Value<1>>().await.unwrap().1, "second");
    assert_eq!(container.get::<Value<2>>().await.unwrap().1, "third");
    assert_eq!(container.get::<Value<3>>().await.unwrap().1, "fourth");
    assert_eq!(container.get::<Value<4>>().await.unwrap().1, "fifth");
    assert_eq!(container.get::<Value<5>>().await.unwrap().1, "sixth");
    assert_eq!(container.get::<Value<6>>().await.unwrap().1, "seventh");
}
