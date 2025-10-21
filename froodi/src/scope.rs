use alloc::vec::Vec;

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

    fn all() -> [Self::Scope; N];
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

impl Scopes<6> for DefaultScope {
    type Scope = Self;

    #[inline]
    fn all() -> [Self; 6] {
        use DefaultScope::{Action, App, Request, Runtime, Session, Step};

        [Runtime, App, Session, Request, Action, Step]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScopeData {
    pub priority: u8,
    pub name: &'static str,
    pub is_skipped_by_default: bool,
}

pub(crate) struct ScopeDataWithChildScopesData<'a> {
    scope_data: Option<&'a ScopeData>,
    child_scopes_data: Vec<&'a ScopeData>,
}

impl<'a> ScopeDataWithChildScopesData<'a> {
    pub fn new(mut scopes: Vec<&'a ScopeData>) -> Self {
        scopes.sort_by_key(|scope| scope.priority);
        let mut iter = scopes.into_iter();
        match iter.next() {
            Some(scope_data) => Self {
                scope_data: Some(scope_data),
                child_scopes_data: iter.collect(),
            },
            None => Self {
                scope_data: None,
                child_scopes_data: Vec::new(),
            },
        }
    }

    pub fn child(&self) -> Self {
        let mut iter = self.child_scopes_data.iter();
        match iter.next() {
            Some(scope_data) => Self {
                scope_data: Some(*scope_data),
                child_scopes_data: iter.map(|val| *val).collect(),
            },
            None => Self {
                scope_data: None,
                child_scopes_data: Vec::new(),
            },
        }
    }
}
