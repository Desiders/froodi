use syn::{
    parse::{Parse, ParseStream},
    Attribute,
};

use crate::attr_parsing::{parse_attrs, Combine};

#[derive(Default)]
pub(super) struct InstantiatorArgs {}

impl Parse for InstantiatorArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let args = InstantiatorArgs::default();

        if !input.is_empty() {
            return Err(input.error("unexpected attribute"));
        }

        Ok(args)
    }
}

impl Combine for InstantiatorArgs {
    fn combine(self, _other: Self) -> syn::Result<Self> {
        Ok(self)
    }
}

pub(crate) fn parse_method_attrs(attrs: &[Attribute]) -> Option<syn::Result<InstantiatorArgs>> {
    parse_attrs("instantiator", attrs)
}
