#![no_std]

extern crate alloc;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::parse::Parse;

mod attr_parsing;
mod injectable;
mod registry;

#[proc_macro_attribute]
pub fn injectable(_attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_with(item, injectable::expand)
}

fn expand_with<F, I, K>(input: TokenStream, f: F) -> TokenStream
where
    F: FnOnce(I) -> syn::Result<K>,
    I: Parse,
    K: ToTokens,
{
    expand(syn::parse(input).and_then(f))
}

fn expand<T>(result: syn::Result<T>) -> TokenStream
where
    T: ToTokens,
{
    match result {
        Ok(tokens) => (quote! { #tokens }).into(),
        Err(err) => err.into_compile_error().into(),
    }
}

#[proc_macro]
pub fn registry(input: TokenStream) -> TokenStream {
    registry::registry(input)
}

#[proc_macro]
pub fn async_registry(input: TokenStream) -> TokenStream {
    registry::async_registry(input)
}
