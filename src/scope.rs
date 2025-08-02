pub trait Scope: Ord {
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
        use DefaultScope::*;

        [Runtime, App, Session, Request, Action, Step]
    }
}
