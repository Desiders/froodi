//! The `registry!` grammar, the same one the `froodi` and `froodi-compile` macros accept.

use syn::{
    bracketed, parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Expr, Ident, Token,
};

/// `factory [, config = expr] [, finalizer = expr]`, options in any order.
pub(crate) struct Entry {
    pub(crate) factory: Expr,
    pub(crate) config: Option<Expr>,
    pub(crate) finalizer: Option<Expr>,
}

impl Parse for Entry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let factory = input.parse()?;
        let (mut config, mut finalizer) = (None, None);
        while input.parse::<Option<Token![,]>>()?.is_some() && !input.is_empty() {
            let name: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let slot = if name == "config" {
                &mut config
            } else if name == "finalizer" {
                &mut finalizer
            } else {
                return Err(syn::Error::new(name.span(), "expected `config = ...` or `finalizer = ...`"));
            };
            if slot.is_some() {
                return Err(syn::Error::new(name.span(), format!("`{name}` is given twice")));
            }
            *slot = Some(input.parse()?);
        }
        Ok(Self {
            factory,
            config,
            finalizer,
        })
    }
}

/// `provide(entry)` inside a `scope(...) [ ... ]` block.
fn parse_scoped_provide(input: ParseStream) -> syn::Result<Entry> {
    let keyword: Ident = input.parse()?;
    if keyword != "provide" {
        return Err(syn::Error::new(keyword.span(), "expected `provide(...)`"));
    }
    let content;
    parenthesized!(content in input);
    content.parse()
}

pub(crate) enum Clause {
    /// `scope(S) [ provide(entry), ... ]`
    Scope { scope: Expr, entries: Vec<Entry> },
    /// `provide(S, entry)`
    Provide { scope: Expr, entry: Box<Entry> },
    /// `extend(registry, ...)`
    Extend(Vec<Expr>),
}

impl Parse for Clause {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let keyword: Ident = input.parse()?;
        let args;
        parenthesized!(args in input);
        if keyword == "scope" {
            let scope = args.parse()?;
            let body;
            bracketed!(body in input);
            let entries = Punctuated::<Entry, Token![,]>::parse_terminated_with(&body, parse_scoped_provide)?;
            Ok(Self::Scope {
                scope,
                entries: entries.into_iter().collect(),
            })
        } else if keyword == "provide" {
            let scope = args.parse()?;
            args.parse::<Token![,]>()?;
            Ok(Self::Provide {
                scope,
                entry: Box::new(args.parse()?),
            })
        } else if keyword == "extend" {
            let registries = Punctuated::<Expr, Token![,]>::parse_terminated(&args)?;
            Ok(Self::Extend(registries.into_iter().collect()))
        } else {
            Err(syn::Error::new(
                keyword.span(),
                "expected `scope(...) [ ... ]`, `provide(...)` or `extend(...)`",
            ))
        }
    }
}

/// The body of one `registry!` or `async_registry!` invocation.
pub(crate) struct RegistryBody {
    pub(crate) clauses: Vec<Clause>,
}

impl Parse for RegistryBody {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let clauses = input.parse_terminated(Clause::parse, Token![,])?;
        Ok(Self {
            clauses: clauses.into_iter().collect(),
        })
    }
}
