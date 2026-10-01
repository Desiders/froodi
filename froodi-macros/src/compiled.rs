//! Balanced typed registry syntax; provider types are inferred by rustc.

use alloc::{boxed::Box, format, vec, vec::Vec};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    bracketed, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Expr, Ident, Token,
};

/// `inst [, config = expr] [, finalizer = expr]`, options in any order.
struct Registration {
    inst: Expr,
    config: Option<Expr>,
    finalizer: Option<Expr>,
}

impl Parse for Registration {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inst = input.parse()?;
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
        Ok(Self { inst, config, finalizer })
    }
}

/// `provide(entry)` inside a `scope(...) [ ... ]` block.
fn parse_scoped_provide(input: ParseStream) -> syn::Result<Registration> {
    let keyword: Ident = input.parse()?;
    if keyword != "provide" {
        return Err(syn::Error::new(keyword.span(), "expected `provide(...)`"));
    }
    let content;
    parenthesized!(content in input);
    content.parse()
}

enum Clause {
    /// `scope(S) [ provide(entry), ... ]`
    Scope { scope: Expr, entries: Vec<Registration> },
    /// `provide(S, entry)`
    Provide { scope: Expr, entry: Box<Registration> },
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
            let entries = Punctuated::<Registration, Token![,]>::parse_terminated_with(&body, parse_scoped_provide)?;
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

struct RegistryInput {
    clauses: Punctuated<Clause, Token![,]>,
}

impl Parse for RegistryInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            clauses: input.parse_terminated(Clause::parse, Token![,])?,
        })
    }
}

/// Builds a balanced `Node` tree over the leaves so trait resolution depth stays logarithmic.
fn balanced(leaves: &[TokenStream2], runtime: &TokenStream2) -> TokenStream2 {
    match leaves {
        [] => quote!(#runtime::Empty),
        [leaf] => leaf.clone(),
        _ => {
            let (left, right) = leaves.split_at(leaves.len() / 2);
            let (left, right) = (balanced(left, runtime), balanced(right, runtime));
            quote!(#runtime::Node(#left, #right))
        }
    }
}

fn leaf(runtime: &TokenStream2, constructor: &TokenStream2, scope: &Expr, entry: &Registration) -> TokenStream2 {
    let inst = &entry.inst;
    let config = entry.config.as_ref().map_or_else(
        || quote!(::core::option::Option::None),
        |config| quote!(::core::option::Option::Some(#config)),
    );
    let finalizer = entry.finalizer.as_ref().map_or_else(
        || quote!(::core::option::Option::None::<#runtime::NoFinalizer>),
        |finalizer| quote!(::core::option::Option::Some(#finalizer)),
    );
    quote!(#constructor(#scope, #inst, #config, #finalizer))
}

fn expand(input: &RegistryInput, runtime: &TokenStream2, constructor: &TokenStream2) -> TokenStream {
    let mut leaves = Vec::new();
    let mut scopes = Vec::new();
    let mut extensions = Vec::new();
    for clause in &input.clauses {
        let (scope, entries) = match clause {
            Clause::Scope { scope, entries } => (scope, entries.iter().collect::<Vec<_>>()),
            Clause::Provide { scope, entry } => (scope, vec![&**entry]),
            Clause::Extend(registries) => {
                extensions.extend(registries);
                continue;
            }
        };
        scopes.push(scope);
        for entry in entries {
            leaves.push(leaf(runtime, constructor, scope, entry));
        }
    }

    // Each extended registry is split into its tree, which joins the leaves, and its scope
    // hierarchy, which is used when the registry declares no scope of its own.
    let bindings: Vec<_> = (0..extensions.len())
        .map(|index| (format_ident!("__froodi_tree_{index}"), format_ident!("__froodi_scopes_{index}")))
        .collect();
    let splits = extensions
        .iter()
        .zip(&bindings)
        .map(|(registry, (tree, scopes))| quote!(let (#tree, #scopes) = #runtime::IntoFragment::into_fragment(#registry);));
    leaves.extend(bindings.iter().map(|(tree, _)| quote!(#tree)));
    let tree = balanced(&leaves, runtime);
    let registry = match bindings.first() {
        None if scopes.is_empty() => quote!(#runtime::Registry::empty()),
        Some((_, first_scopes)) if scopes.is_empty() => {
            quote!(#runtime::Registry::from_parts(#tree, #first_scopes))
        }
        _ => quote!(#runtime::Registry::from_tree(#tree, [#(#scopes),*])),
    };
    quote!({
        #(#splits)*
        #registry
    })
    .into()
}

struct NativeRegistryInput {
    runtime: TokenStream2,
    registry: RegistryInput,
}

impl Parse for NativeRegistryInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut runtime = TokenStream2::new();
        while !input.peek(Token![;]) {
            runtime.extend([input.parse::<proc_macro2::TokenTree>()?]);
        }
        input.parse::<Token![;]>()?;
        Ok(Self {
            runtime,
            registry: input.parse()?,
        })
    }
}

pub fn registry(input: TokenStream) -> TokenStream {
    let NativeRegistryInput { runtime, registry } = parse_macro_input!(input as NativeRegistryInput);
    expand(&registry, &runtime, &quote!(#runtime::reg))
}

pub fn async_registry(input: TokenStream) -> TokenStream {
    let NativeRegistryInput { runtime, registry } = parse_macro_input!(input as NativeRegistryInput);
    expand(&registry, &runtime, &quote!(#runtime::async_reg))
}
