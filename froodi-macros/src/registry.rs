//! Balanced typed registry syntax; provider types are inferred by rustc.

use alloc::{boxed::Box, format, string::ToString, vec, vec::Vec};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote, quote_spanned};
use syn::{
    bracketed, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    spanned::Spanned,
    Expr, Ident, Token, Type,
};

/// `inst [, config = expr] [, finalizer = expr]`, options in any order.
struct Registration {
    kind: RegistrationKind,
    options: Vec<RegistrationOption>,
}

enum RegistrationKind {
    Provide(Expr),
    Construct(Type),
}

enum RegistrationOption {
    Config(Expr),
    Finalizer(Expr),
}

impl Parse for Registration {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inst = input.parse()?;
        Ok(Self {
            kind: RegistrationKind::Provide(inst),
            options: parse_options(input, true)?,
        })
    }
}

fn parse_options(input: ParseStream, require_first_comma: bool) -> syn::Result<Vec<RegistrationOption>> {
    let mut options = Vec::new();
    let mut first = true;
    while !input.is_empty() {
        if !first || require_first_comma {
            input.parse::<Token![,]>()?;
        }
        if input.is_empty() {
            break;
        }
        let name: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        if name != "config" && name != "finalizer" {
            return Err(syn::Error::new(name.span(), "expected `config = ...` or `finalizer = ...`"));
        }
        if options.iter().any(|option| match option {
            RegistrationOption::Config(_) => name == "config",
            RegistrationOption::Finalizer(_) => name == "finalizer",
        }) {
            return Err(syn::Error::new(name.span(), format!("`{name}` is given twice")));
        }
        let value = input.parse()?;
        options.push(if name == "config" {
            RegistrationOption::Config(value)
        } else {
            RegistrationOption::Finalizer(value)
        });
        first = false;
    }
    Ok(options)
}

fn construct_type(input: ParseStream) -> syn::Result<Type> {
    input.parse::<Token![::]>()?;
    input.parse::<Token![<]>()?;
    let ty = input.parse()?;
    input.parse::<Token![>]>()?;
    Ok(ty)
}

/// `provide(entry)` or `construct::<T>()` inside a scope block.
fn parse_scoped_provide(input: ParseStream) -> syn::Result<Registration> {
    let keyword: Ident = input.parse()?;
    if keyword == "construct" {
        let ty = construct_type(input)?;
        let content;
        parenthesized!(content in input);
        return Ok(Registration {
            kind: RegistrationKind::Construct(ty),
            options: parse_options(&content, false)?,
        });
    }
    if keyword != "provide" {
        return Err(syn::Error::new(keyword.span(), "expected `provide(...)` or `construct::<T>()`"));
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
        if keyword == "construct" {
            return Err(syn::Error::new(
                keyword.span(),
                "put `construct::<T>(...)` inside `scope(...) [ ... ]`",
            ));
        }
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

fn leaf(runtime: &TokenStream2, scope: &Ident, entry: &Registration) -> TokenStream2 {
    let inst = match &entry.kind {
        RegistrationKind::Construct(ty) => {
            let description = quote_spanned!(ty.span()=> ::core::concat!("construct::<", ::core::stringify!(#ty), ">() at ", ::core::file!(), ":", ::core::line!(), ":", ::core::column!()));
            return registration_leaf(runtime, scope, entry, quote!(#runtime::construct::<#ty>()), description);
        }
        RegistrationKind::Provide(inst) => inst,
    };
    let label = match inst {
        Expr::Path(path) => quote!(#path).to_string(),
        Expr::Call(call) => {
            let func = &call.func;
            format!("{}(...)", quote!(#func))
        }
        Expr::Closure(closure) => {
            // Show parameters, without exposing literals or verbose instantiator bodies.
            let mut signature = closure.clone();
            *signature.body = syn::parse_quote!({ __froodi_body() });
            let file = syn::parse_quote! {
                fn __froodi_diagnostic() {
                    let _ = #signature;
                }
            };
            let formatted = prettyplease::unparse(&file).split_whitespace().collect::<Vec<_>>().join(" ");
            formatted
                .strip_prefix("fn __froodi_diagnostic() { let _ = ")
                .and_then(|signature| signature.strip_suffix(" { __froodi_body() }; }"))
                .map_or_else(
                    || {
                        let inputs = &closure.inputs;
                        format!("|{}| ...", quote!(#inputs))
                    },
                    |signature| format!("{signature} ..."),
                )
        }
        _ => "instantiator expression".into(),
    };
    let label = label.replace(" :: ", "::");
    let description = quote_spanned!(inst.span()=> ::core::concat!("provide(", #label, ") at ", ::core::file!(), ":", ::core::line!(), ":", ::core::column!()));
    registration_leaf(runtime, scope, entry, quote!(#inst), description)
}

fn registration_leaf(
    runtime: &TokenStream2,
    scope: &Ident,
    entry: &Registration,
    inst_value: TokenStream2,
    description: TokenStream2,
) -> TokenStream2 {
    let source = format_ident!("__FroodiRegistrationSource", span = Span::mixed_site());
    let inst_binding = format_ident!("__froodi_inst", span = Span::mixed_site());
    let config_binding = format_ident!("__froodi_config", span = Span::mixed_site());
    let finalizer_binding = format_ident!("__froodi_finalizer", span = Span::mixed_site());
    let mut bindings = Vec::new();
    let mut config = quote!(::core::option::Option::None);
    let mut finalizer = quote!(::core::option::Option::None::<#runtime::NoFinalizer>);
    for option in &entry.options {
        match option {
            RegistrationOption::Config(value) => {
                bindings.push(quote!(let #config_binding = #value;));
                config = quote!(::core::option::Option::Some(#config_binding));
            }
            RegistrationOption::Finalizer(value) => {
                bindings.push(quote!(let #finalizer_binding = #value;));
                finalizer = quote!(::core::option::Option::Some(#finalizer_binding));
            }
        }
    }
    quote!({
        struct #source;
        impl #runtime::RegistrationSource for #source {
            const DESCRIPTION: &'static str = #description;
        }
        let #inst_binding = #inst_value;
        #(#bindings)*
        #scope.locate::<_, #source>(#runtime::reg(#scope.data, #inst_binding, #config, #finalizer))
    })
}

fn expand(input: &RegistryInput, runtime: &TokenStream2) -> TokenStream {
    let mut leaves = Vec::new();
    let mut first_scopes = None;
    let mut extensions = Vec::new();
    let mut bindings = Vec::new();
    let scope_converter = format_ident!("__froodi_scope_converter", span = Span::mixed_site());
    for clause in &input.clauses {
        let (scope, entries) = match clause {
            Clause::Scope { scope, entries } => (scope, entries.iter().collect::<Vec<_>>()),
            Clause::Provide { scope, entry } => (scope, vec![&**entry]),
            Clause::Extend(registries) => {
                for registry in registries {
                    let index = extensions.len();
                    let tree = format_ident!("__froodi_tree_{index}", span = Span::mixed_site());
                    let scopes = format_ident!("__froodi_extended_scopes_{index}", span = Span::mixed_site());
                    bindings.push(quote!(let (#tree, #scopes) = #runtime::IntoFragment::into_fragment(#registry);));
                    extensions.push((tree, scopes));
                }
                continue;
            }
        };
        let index = leaves.len();
        let converted_scope = format_ident!("__froodi_scope_{index}", span = Span::mixed_site());
        let raw_scope = format_ident!("__froodi_raw_scope_{index}", span = Span::mixed_site());
        let scope_type = format_ident!("__froodi_scope_type_{index}", span = Span::mixed_site());
        bindings.push(quote! {
            let #raw_scope = #scope;
            let #scope_type = #runtime::ScopeType::new(&#raw_scope);
        });
        if first_scopes.is_none() {
            let scopes_binding = format_ident!("__froodi_scopes", span = Span::mixed_site());
            bindings.push(quote!(use #runtime::ClassifyScope as _;));
            bindings.push(quote!(let (#converted_scope, #scopes_binding, #scope_converter) = #runtime::Registry::scope_data(#raw_scope);));
            first_scopes = Some(scopes_binding);
        } else {
            bindings.push(quote!(let #converted_scope = #scope_converter.convert(#raw_scope);));
        }
        bindings.push(quote! {
            let #converted_scope = (&&#scope_type).classify(#converted_scope);
        });
        for entry in entries {
            let value = leaf(runtime, &converted_scope, entry);
            let binding = format_ident!("__froodi_registration_{}", leaves.len(), span = Span::mixed_site());
            bindings.push(quote!(let #binding = #value;));
            leaves.push(quote!(#binding));
        }
    }

    leaves.extend(extensions.iter().map(|(tree, _)| quote!(#tree)));
    let tree = balanced(&leaves, runtime);
    let scopes = first_scopes.or_else(|| extensions.first().map(|(_, scopes)| scopes.clone()));
    let registry = match scopes {
        None => quote!(#runtime::Registry::empty()),
        Some(scopes) => quote!(#runtime::Registry::from_parts(#tree, #scopes)),
    };
    quote!({
        #(#bindings)*
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
    expand(&registry, &runtime)
}
