//! Froodi's scope model, unchanged: `Scope`, `Scopes<N>`, `DefaultScope` and `ScopeData`.

use core::fmt::{self, Display, Formatter};

pub trait Scope: Ord + Into<ScopeData> {
    #[must_use]
    fn name(&self) -> &'static str;

    #[must_use]
    fn priority(&self) -> u8;

    #[must_use]
    fn is_skipped_by_default(&self) -> bool {
        false
    }
}

pub trait Scopes<const N: usize> {
    type Scope;

    fn all() -> (Self::Scope, [Self::Scope; N]);
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum DefaultScope {
    Runtime,
    App,
    Session,
    Request,
    Action,
    Step,
}

impl From<DefaultScope> for ScopeData {
    fn from(scope: DefaultScope) -> Self {
        Self {
            priority: scope.priority(),
            name: scope.name(),
            is_skipped_by_default: scope.is_skipped_by_default(),
        }
    }
}

impl Scope for DefaultScope {
    #[inline]
    fn name(&self) -> &'static str {
        match self {
            DefaultScope::Runtime => "runtime",
            DefaultScope::App => "app",
            DefaultScope::Session => "session",
            DefaultScope::Request => "request",
            DefaultScope::Action => "action",
            DefaultScope::Step => "step",
        }
    }

    #[inline]
    fn priority(&self) -> u8 {
        *self as u8
    }

    #[inline]
    fn is_skipped_by_default(&self) -> bool {
        matches!(self, DefaultScope::Runtime | DefaultScope::Session)
    }
}

impl Scopes<5> for DefaultScope {
    type Scope = Self;

    #[inline]
    fn all() -> (Self::Scope, [Self::Scope; 5]) {
        use DefaultScope::{Action, App, Request, Runtime, Session, Step};

        (Runtime, [App, Session, Request, Action, Step])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScopeData {
    pub priority: u8,
    pub name: &'static str,
    pub is_skipped_by_default: bool,
}

impl Display for ScopeData {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}, is_skipped_by_default = {})",
            self.name, self.priority, self.is_skipped_by_default
        )
    }
}

impl From<ScopeData> for froodi_compile_core::ScopeKey {
    #[inline]
    fn from(scope: ScopeData) -> Self {
        Self {
            priority: scope.priority,
            name: scope.name,
            skipped_by_default: scope.is_skipped_by_default,
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::DefaultScope::*;
    use super::{DefaultScope, Scope, ScopeData, Scopes};
    use alloc::{format, string::ToString as _};

    #[test]
    fn test_scope_attributes() {
        assert_eq!(Runtime.name(), "runtime");
        assert_eq!(App.name(), "app");
        assert_eq!(Session.name(), "session");
        assert_eq!(Request.name(), "request");
        assert_eq!(Action.name(), "action");
        assert_eq!(Step.name(), "step");

        assert_eq!(Runtime.priority(), 0);
        assert_eq!(App.priority(), 1);
        assert_eq!(Session.priority(), 2);
        assert_eq!(Request.priority(), 3);
        assert_eq!(Action.priority(), 4);
        assert_eq!(Step.priority(), 5);

        assert_eq!(Runtime.priority(), Runtime as u8);
        assert_eq!(App.priority(), App as u8);
        assert_eq!(Session.priority(), Session as u8);
        assert_eq!(Request.priority(), Request as u8);
        assert_eq!(Action.priority(), Action as u8);
        assert_eq!(Step.priority(), Step as u8);

        assert!(Runtime.is_skipped_by_default());
        assert!(!App.is_skipped_by_default());
        assert!(Session.is_skipped_by_default());
        assert!(!Request.is_skipped_by_default());
        assert!(!Action.is_skipped_by_default());
        assert!(!Step.is_skipped_by_default());
    }

    #[test]
    fn test_scope_ord_eq_copy() {
        assert!(Runtime < App);
        assert!(App < Session);
        assert!(Session < Request);
        assert!(Request < Action);
        assert!(Action < Step);

        assert!(Runtime < Step);
        assert!(Step > Runtime);

        // DefaultScope does not derive Debug, so compare with `==` instead of assert_eq!.
        assert!(App == App);
        assert!(App != Request);

        let a = Request;
        let b = a;
        assert!(a == b);
        assert_eq!(a.priority(), 3);
    }

    #[test]
    fn test_from_scope_for_scope_data() {
        let app = ScopeData::from(App);
        assert_eq!(app.priority, 1);
        assert_eq!(app.name, "app");
        assert!(!app.is_skipped_by_default);
        assert_eq!(
            app,
            ScopeData {
                priority: 1,
                name: "app",
                is_skipped_by_default: false,
            }
        );

        let runtime = ScopeData::from(Runtime);
        assert_eq!(runtime.priority, 0);
        assert_eq!(runtime.name, "runtime");
        assert!(runtime.is_skipped_by_default);
        assert_eq!(
            runtime,
            ScopeData {
                priority: 0,
                name: "runtime",
                is_skipped_by_default: true,
            }
        );
    }

    #[test]
    fn test_scope_data_display() {
        assert_eq!(format!("{}", ScopeData::from(App)), "app (1, is_skipped_by_default = false)");
        assert_eq!(format!("{}", ScopeData::from(Runtime)), "runtime (0, is_skipped_by_default = true)");
        let runtime = ScopeData::from(Runtime).to_string();
        assert!(runtime.contains("runtime (0, is_skipped_by_default = true)"));
    }

    #[test]
    fn test_scopes_all() {
        let (root, children) = DefaultScope::all();
        assert!(root == Runtime);
        assert!(children == [App, Session, Request, Action, Step]);
    }
}
