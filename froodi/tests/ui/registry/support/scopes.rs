#![allow(dead_code)]

use super::scope_api::{Scope, ScopeData, Scopes, StaticScope};

macro_rules! static_scope {
    ($scope:ident, $priority:expr, $name:literal, $skip:expr) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $scope;

        impl From<$scope> for ScopeData {
            fn from(_: $scope) -> Self {
                $scope::DATA
            }
        }

        impl Scopes<3> for $scope {
            type Scope = ScopeData;

            fn all() -> (Self::Scope, [Self::Scope; 3]) {
                (Root::DATA, [App::DATA, Request::DATA, Transaction::DATA])
            }
        }

        impl StaticScope for $scope {
            const DATA: ScopeData = ScopeData {
                priority: $priority,
                name: $name,
                is_skipped_by_default: $skip,
            };
        }
    };
}

static_scope!(Root, 0, "root", true);
static_scope!(App, 1, "app", false);
static_scope!(Request, 3, "request", false);
static_scope!(Transaction, 4, "transaction", false);
static_scope!(EqualApp, 1, "equal app", false);

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeScope(pub u8);

impl Scope for RuntimeScope {
    fn name(&self) -> &'static str {
        "runtime value"
    }

    fn priority(&self) -> u8 {
        self.0
    }
}

impl From<RuntimeScope> for ScopeData {
    fn from(scope: RuntimeScope) -> Self {
        Self {
            priority: scope.0,
            name: scope.name(),
            is_skipped_by_default: false,
        }
    }
}

impl Scopes<3> for RuntimeScope {
    type Scope = ScopeData;

    fn all() -> (Self::Scope, [Self::Scope; 3]) {
        Root::all()
    }
}
