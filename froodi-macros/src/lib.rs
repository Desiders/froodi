#![no_std]

extern crate alloc;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::parse::Parse;

mod attr_parsing;
#[cfg(feature = "compiled")]
mod compiled;
mod injectable;

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

#[cfg(feature = "compiled")]
#[proc_macro]
pub fn compiled_registry(input: TokenStream) -> TokenStream {
    compiled::registry(input)
}

#[cfg(feature = "compiled")]
#[proc_macro]
pub fn compiled_async_registry(input: TokenStream) -> TokenStream {
    compiled::async_registry(input)
}
