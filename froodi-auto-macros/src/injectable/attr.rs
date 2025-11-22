use syn::{
    parse::{Parse, ParseStream},
    Attribute, Expr, Token,
};

use crate::attr_parsing::{combine_attribute, parse_assignment_attribute, parse_attrs, Combine};

pub(crate) mod kw {
    syn::custom_keyword!(finalizer);
    syn::custom_keyword!(config);
}

pub(crate) struct ProvideArgs {
    pub(super) scope: Expr,
    pub(super) finalizer: Option<(kw::finalizer, Expr)>,
    pub(super) config: Option<(kw::config, Expr)>,
}

impl Parse for ProvideArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(syn::Ident) && input.peek2(Token![=]) {
            return Err(input.error("expected positional scope argument"));
        }

        let scope = input.parse::<Expr>()?;
        let mut finalizer = None;
        let mut config = None;

        let _ = input.parse::<Token![,]>();

        while !input.is_empty() {
            let lh = input.lookahead1();
            if lh.peek(kw::finalizer) {
                parse_assignment_attribute(input, &mut finalizer)?;
            } else if lh.peek(kw::config) {
                parse_assignment_attribute(input, &mut config)?;
            } else {
                return Err(lh.error());
            }

            let _ = input.parse::<Token![,]>();
        }

        Ok(Self { scope, finalizer, config })
    }
}

impl Combine for ProvideArgs {
    fn combine(mut self, other: Self) -> syn::Result<Self> {
        let Self { finalizer, config, .. } = other;
        combine_attribute(&mut self.finalizer, finalizer)?;
        combine_attribute(&mut self.config, config)?;
        Ok(self)
    }
}

pub(crate) fn parse_method_attrs(attrs: &[Attribute]) -> Option<syn::Result<ProvideArgs>> {
    parse_attrs("provide", attrs).map(|result| result.map_err(|(err, attr)| syn::Error::new_spanned(attr, err)))
}
