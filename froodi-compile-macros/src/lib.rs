//! `registry!` frontend of the experimental compile-time Froodi engine.
//!
//! The macro keeps Froodi's registry syntax and turns it into a balanced tree of typed
//! registrations. It never needs to know the provided or dependency types: those stay in the
//! factory's type and are checked by rustc.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    bracketed, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Expr, Ident, Token,
};

struct Provide {
    factory: Expr,
}

impl Parse for Provide {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let keyword: Ident = input.parse()?;
        if keyword != "provide" {
            return Err(syn::Error::new(keyword.span(), "expected `provide(...)`"));
        }
        let content;
        parenthesized!(content in input);
        Ok(Self { factory: content.parse()? })
    }
}

struct ScopeClause {
    scope: Expr,
    provides: Punctuated<Provide, Token![,]>,
}

impl Parse for ScopeClause {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let keyword: Ident = input.parse()?;
        if keyword != "scope" {
            return Err(syn::Error::new(keyword.span(), "expected `scope(...) [ ... ]`"));
        }
        let scope_content;
        parenthesized!(scope_content in input);
        let entries;
        bracketed!(entries in input);
        Ok(Self {
            scope: scope_content.parse()?,
            provides: entries.parse_terminated(Provide::parse, Token![,])?,
        })
    }
}

struct RegistryInput {
    clauses: Punctuated<ScopeClause, Token![,]>,
}

impl Parse for RegistryInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            clauses: input.parse_terminated(ScopeClause::parse, Token![,])?,
        })
    }
}

/// Builds a balanced `Node` tree over the leaves so trait resolution depth stays logarithmic.
fn balanced(leaves: &[TokenStream2]) -> TokenStream2 {
    match leaves {
        [] => unreachable!("a registry has at least one leaf"),
        [leaf] => leaf.clone(),
        _ => {
            let (left, right) = leaves.split_at(leaves.len() / 2);
            let (left, right) = (balanced(left), balanced(right));
            quote!(::froodi_compile::__private::Node(#left, #right))
        }
    }
}

#[proc_macro]
pub fn registry(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as RegistryInput);
    let mut leaves = Vec::new();
    for clause in &input.clauses {
        let scope = &clause.scope;
        for provide in &clause.provides {
            let factory = &provide.factory;
            leaves.push(quote!(::froodi_compile::__private::reg(#scope, #factory)));
        }
    }
    let tree = balanced(&leaves);
    quote!(::froodi_compile::__private::Registry::from_tree(#tree)).into()
}
