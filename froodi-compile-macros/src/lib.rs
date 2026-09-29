//! `registry!` frontend of the experimental compile-time Froodi engine.
//!
//! The macro keeps Froodi's registry syntax and turns it into a balanced tree of typed
//! registrations. It never needs to know the provided or dependency types: those stay in the
//! factory's type and are checked by rustc.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, quote_spanned};
use syn::{
    bracketed, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    spanned::Spanned,
    Expr, Ident, Token,
};

/// `factory [, config = expr] [, finalizer = expr]`, options in any order.
struct Entry {
    factory: Expr,
    config: Option<Expr>,
    finalizer: Option<Expr>,
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

enum Clause {
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
fn balanced(leaves: &[TokenStream2]) -> TokenStream2 {
    match leaves {
        [] => quote!(::froodi_compile::__private::Empty),
        [leaf] => leaf.clone(),
        _ => {
            let (left, right) = leaves.split_at(leaves.len() / 2);
            let (left, right) = (balanced(left), balanced(right));
            quote!(::froodi_compile::__private::Node(#left, #right))
        }
    }
}

/// `instance(value)` is recognised by name: its value exists only at runtime and it has no
/// dependencies. Anything else is an ordinary factory.
fn value_source(factory: &Expr) -> TokenStream2 {
    let is_instance = matches!(factory, Expr::Call(call) if matches!(&*call.func, Expr::Path(path)
        if path.path.segments.last().is_some_and(|segment| segment.ident == "instance")));
    if is_instance {
        quote!(::froodi_compile::__private::ValueSource::Instance)
    } else {
        quote!(::froodi_compile::__private::ValueSource::Factory)
    }
}

/// Where the factory expression is written. Used only in diagnostics.
fn origin(factory: &Expr) -> TokenStream2 {
    let span = factory.span();
    let expr = span.source_text().unwrap_or_else(|| quote!(#factory).to_string());
    quote_spanned! {span=>
        ::froodi_compile::__private::Origin {
            expr: #expr,
            file: ::core::file!(),
            line: ::core::line!(),
            column: ::core::column!(),
        }
    }
}

fn leaf(scope: &Expr, entry: &Entry) -> TokenStream2 {
    let factory = &entry.factory;
    let source = value_source(factory);
    let origin = origin(factory);
    let config = entry.config.as_ref().map_or_else(
        || quote!(::core::option::Option::None),
        |config| quote!(::core::option::Option::Some(#config)),
    );
    let finalizer = entry.finalizer.as_ref().map_or_else(
        || quote!(::froodi_compile::__private::NoFinalizer),
        |finalizer| quote!(::froodi_compile::__private::WithFinalizer(#finalizer)),
    );
    quote!(::froodi_compile::__private::reg(#scope, #factory, #config, #finalizer, #source, #origin))
}

#[proc_macro]
pub fn registry(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as RegistryInput);
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
            leaves.push(leaf(scope, entry));
        }
    }

    // Each extended registry is split into its tree, which joins the leaves, and its scope
    // hierarchy, which is used when the registry declares no scope of its own.
    let bindings: Vec<_> = (0..extensions.len())
        .map(|index| (format_ident!("__froodi_tree_{index}"), format_ident!("__froodi_scopes_{index}")))
        .collect();
    let splits = extensions.iter().zip(&bindings).map(
        |(registry, (tree, scopes))| quote!(let (#tree, #scopes) = ::froodi_compile::__private::IntoFragment::into_fragment(#registry);),
    );
    leaves.extend(bindings.iter().map(|(tree, _)| quote!(#tree)));
    let tree = balanced(&leaves);
    let registry = match bindings.first() {
        None if scopes.is_empty() => quote!(::froodi_compile::__private::Registry::empty()),
        Some((_, first_scopes)) if scopes.is_empty() => {
            quote!(::froodi_compile::__private::Registry::from_parts(#tree, #first_scopes))
        }
        _ => quote!(::froodi_compile::__private::Registry::from_tree(#tree, [#(#scopes),*])),
    };
    quote!({
        #(#splits)*
        #registry
    })
    .into()
}
