//! Named registry templates. Rust infers argument and provider types at composition.

extern crate std;

use alloc::{format, string::ToString, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};
use proc_macro2::{Span, TokenStream, TokenTree};
use quote::{format_ident, quote, quote_spanned};
use std::env;
use syn::{
    parse::{Parse, ParseStream},
    Ident, ItemMacro, LitStr, Token, Visibility,
};

use crate::registry::RegistryInput;

// Only allocates generated macro names; no fragment lookup or state is stored.
static NEXT_EXPORT: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct Fragment {
    visibility: Visibility,
    name: Ident,
    parameters: Vec<Ident>,
}

impl Parse for Fragment {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let visibility = input.parse()?;
        let name = input.parse()?;
        if input.peek(Token![<]) {
            return Err(input.error("fragments do not have generic or lifetime parameters; pass expressions instead"));
        }
        let mut parameters = Vec::new();
        if input.peek(syn::token::Paren) {
            let arguments;
            syn::parenthesized!(arguments in input);
            while !arguments.is_empty() {
                let parameter: Ident = arguments
                    .parse()
                    .map_err(|_| arguments.error("fragment parameters must be identifiers, for example `configuration(cfg, url)`"))?;
                if parameters.contains(&parameter) {
                    return Err(syn::Error::new_spanned(parameter, "fragment parameter is given twice"));
                }
                parameters.push(parameter);
                if arguments.is_empty() {
                    break;
                }
                if !arguments.peek(Token![,]) {
                    return Err(arguments.error("fragment parameters are untyped identifiers; use `cfg, url`"));
                }
                arguments.parse::<Token![,]>()?;
            }
        }
        Ok(Self {
            visibility,
            name,
            parameters,
        })
    }
}

pub(crate) fn expand(fragment: Fragment, input: TokenStream) -> syn::Result<TokenStream> {
    let item: ItemMacro =
        syn::parse2(input).map_err(|_| syn::Error::new(fragment.name.span(), "fragment must annotate a `registry! { ... }` invocation"))?;
    if item.ident.is_some() || item.mac.path.segments.last().map_or(true, |segment| segment.ident != "registry") {
        return Err(syn::Error::new_spanned(
            item.mac.path,
            "fragment must annotate a `registry! { ... }` invocation",
        ));
    }
    let body: RegistryInput = syn::parse2(item.mac.tokens)?;
    let body = definition_crate(quote!(#body));
    let name = &fragment.name;
    let sequence = NEXT_EXPORT.fetch_add(1, Ordering::Relaxed);
    let implementation = format_ident!("__froodi_fragment_{name}_{sequence}", span = Span::mixed_site());
    // Distinguish equal names in different crates and repeated declaration expansion.
    let package = env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let identity = LitStr::new(&format!("{package}:{implementation}:{:?}", name.span()), name.span());
    let origin = LitStr::new(&name.to_string(), name.span());
    let visibility = &fragment.visibility;
    let export = matches!(visibility, Visibility::Public(_)).then(|| quote!(#[macro_export]));
    let attributes = &item.attrs;
    let registry_path = &item.mac.path;
    let fresh = |prefix: &str| {
        let mut name = format_ident!("__froodi_fragment_{prefix}_{sequence}", span = Span::mixed_site());
        while fragment.parameters.contains(&name) {
            name = format_ident!("{name}_", span = Span::mixed_site());
        }
        name
    };
    let runtime = fresh("runtime");
    let template = fresh("template");
    let invalid = fresh("invalid");
    let arguments = fragment
        .parameters
        .iter()
        .enumerate()
        .map(|(index, _)| fresh(&format!("argument_{index}")))
        .collect::<Vec<_>>();
    let matchers = arguments.iter().map(|argument| quote!($ #argument : expr));
    let bindings = fragment
        .parameters
        .iter()
        .zip(&arguments)
        .map(|(parameter, argument)| quote!(let #parameter = #argument;));
    let error = format!("fragment {name}: expected {} argument(s)", arguments.len());
    let misuse = format!("fragment `{name}` can only be expanded inside `registry!` with `extend_fragment(...)`");
    Ok(quote! {
        #(#attributes)*
        #[doc(hidden)]
        use #registry_path as _;

        #(#attributes)*
        #[doc(hidden)]
        #[allow(unused_macros)]
        #export
        macro_rules! #implementation {
            (@froodi_fragment (#(#matchers),* $(,)?)
                [$($ #runtime:tt)*] [$($ #template:tt)*]) => {
                $($ #runtime)*::registry! {
                    $($ #runtime)*;
                    @resume [$($ #template)*]
                    { #(let #arguments = $ #arguments;)* }
                    { #(#bindings)* }
                    [#body] #origin #identity
                }
            };
            (@froodi_fragment $($ #invalid:tt)*) => { ::core::compile_error!(#error); };
            ($($ #invalid:tt)*) => { ::core::compile_error!(#misuse); };
        }
        #(#attributes)*
        #[doc(inline)]
        #[allow(unused_imports)]
        #visibility use #implementation as #name;
    })
}

// Explicit `crate::` paths belong to the declaration crate. Bare names retain
// macro_rules' invocation-site lookup; registry-local imports remain lexical.
fn definition_crate(tokens: TokenStream) -> TokenStream {
    let mut output = TokenStream::new();
    let mut tokens = tokens.into_iter().peekable();
    while let Some(token) = tokens.next() {
        match token {
            TokenTree::Ident(ident)
                if ident == "crate" && matches!(tokens.peek(), Some(TokenTree::Punct(punctuation)) if punctuation.as_char() == ':') =>
            {
                output.extend(quote_spanned!(ident.span()=> $crate));
            }
            TokenTree::Group(group) => {
                let mut replacement = proc_macro2::Group::new(group.delimiter(), definition_crate(group.stream()));
                replacement.set_span(group.span());
                output.extend([TokenTree::Group(replacement)]);
            }
            token => output.extend([token]),
        }
    }
    output
}
