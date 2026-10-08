use super::registration::RegistrationSource;
use crate::{Scope, ScopeData, StaticScope};
use core::marker::PhantomData;

pub struct DynamicScope;

pub struct StaticRegistrationSource<Source, S>(PhantomData<fn(S) -> (Source, S)>);

impl<Source: RegistrationSource, S: StaticScope> RegistrationSource for StaticRegistrationSource<Source, S> {
    const DESCRIPTION: &'static str = Source::DESCRIPTION;
    const SCOPE: Option<&'static ScopeData> = Some(&S::DATA);
}

pub struct ScopeType<S>(PhantomData<fn(S) -> S>);

impl<S> ScopeType<S> {
    #[must_use]
    pub fn new(_: &S) -> Self {
        Self(PhantomData)
    }
}

pub struct TypedScopeData<Kind> {
    pub data: ScopeData,
    marker: PhantomData<fn() -> Kind>,
}

impl<Kind> Copy for TypedScopeData<Kind> {}

impl<Kind> Clone for TypedScopeData<Kind> {
    fn clone(&self) -> Self {
        *self
    }
}

pub trait ClassifyScope {
    type Kind;

    fn classify(self, data: ScopeData) -> TypedScopeData<Self::Kind>;
}

impl<S: Scope> ClassifyScope for &ScopeType<S> {
    type Kind = DynamicScope;

    fn classify(self, data: ScopeData) -> TypedScopeData<DynamicScope> {
        TypedScopeData { data, marker: PhantomData }
    }
}

impl<S: StaticScope> ClassifyScope for &&ScopeType<S> {
    type Kind = ScopeType<S>;

    fn classify(self, data: ScopeData) -> TypedScopeData<Self::Kind> {
        assert_eq!(data, S::DATA, "StaticScope metadata differs from Scope conversion");
        TypedScopeData { data, marker: PhantomData }
    }
}
