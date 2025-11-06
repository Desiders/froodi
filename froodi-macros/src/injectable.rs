mod attr;

use crate::injectable::attr::{parse_method_attrs, ProvideArgs};

use proc_macro2::{Span, TokenStream};
use proc_macro_error::abort;
use quote::{format_ident, quote, quote_spanned, ToTokens as _};
use syn::{
    spanned::Spanned as _,
    FnArg, Ident,
    ImplItem::Fn,
    ImplItemFn,
    Item::{self, Impl},
    ItemImpl, ReturnType, Type,
};

fn expand_instantiator_impl_for_method(impl_item_fn: &ImplItemFn, self_ty: &Type, provide_args: ProvideArgs) -> TokenStream {
    let ref fn_name = impl_item_fn.sig.ident;
    let is_async = impl_item_fn.sig.asyncness.is_some();
    let impl_span = impl_item_fn.span();

    if let ReturnType::Default = impl_item_fn.sig.output {
        abort!(fn_name, "method must have an explicit return type");
    }

    let types_with_spans = impl_item_fn
        .sig
        .inputs
        .iter()
        .map(|input| match input {
            FnArg::Typed(pat_type) => (pat_type.ty.as_ref(), pat_type.ty.span()),
            FnArg::Receiver(_) => abort!(input, "methods with `self` are not supported"),
        })
        .collect::<Box<[_]>>();
    let types = types_with_spans.iter().map(|(ty, _)| *ty).collect::<Box<[_]>>();

    let (names, names_with_parenthesis) = if types.is_empty() {
        (quote! {}, quote! { () })
    } else {
        let idents = types
            .iter()
            .enumerate()
            .map(|(i, _)| Ident::new(&format!("arg{i}"), Span::call_site()))
            .collect::<Box<[_]>>();
        let names = quote! { #( #idents ),* };
        let names_with_parenthesis = quote! { ( #( #idents ),* ) };
        (names, names_with_parenthesis)
    };

    let types_with_parenthesis = if types.is_empty() {
        quote! { () }
    } else {
        let quoted_types = types_with_spans.iter().map(|(ty, span)| {
            quote_spanned! { *span => #ty }
        });
        quote! { ( #( #quoted_types ),* ) }
    };

    let scope = provide_args.scope;
    let config = match provide_args.config {
        Some((_, config)) => quote_spanned! { config.span() => Some(#config) },
        None => quote! { None },
    };
    let finalizer = match provide_args.finalizer {
        Some((_, finalizer)) => quote_spanned! { finalizer.span() => Some(#finalizer) },
        None => {
            if impl_item_fn.sig.asyncness.is_some() {
                quote! { None::<::froodi::macros_utils::async_impl::FinDummy<_>> }
            } else {
                quote! { None::<::froodi::macros_utils::sync::FinDummy<_>> }
            }
        }
    };

    let self_ty_uppercase = self_ty.into_token_stream().to_string().to_uppercase();
    let fn_name_uppercase = fn_name.to_string().to_uppercase();
    let autowired_struct_name = format_ident!("__Autowired{}_{}", self_ty_uppercase, fn_name_uppercase);
    let global_entry_getter_name = format_ident!("__{}_{}", self_ty_uppercase, fn_name_uppercase);

    let instantiate_function_quote = if is_async {
        quote_spanned! { impl_span =>
            async fn instantiate(&mut self, #names_with_parenthesis: #types_with_parenthesis) -> Result<Self::Provides, Self::Error> {
                #self_ty::#fn_name(#names).await
            }
        }
    } else {
        quote_spanned! { impl_span =>
            fn instantiate(&mut self, #names_with_parenthesis: #types_with_parenthesis) -> Result<Self::Provides, Self::Error> {
                #self_ty::#fn_name(#names)
            }
        }
    };
    let dependencies_quote = types_with_spans
        .iter()
        .map(|(ty, span)| {
            quote_spanned! { *span =>
                ::froodi::Dependency {
                    type_info: <#ty as ::froodi::DependencyResolver>::type_info()
                }
            }
        })
        .collect::<Box<[_]>>();
    let dependencies_function_quote = quote_spanned! { impl_span =>
        fn dependencies() -> ::froodi::macros_utils::aliases::BTreeSet<::froodi::Dependency> {
            ::froodi::macros_utils::aliases::BTreeSet::from_iter([
                #( #dependencies_quote, )*
            ])
        }
    };
    let instantiator_impl_quote = if is_async {
        quote_spanned! { impl_span =>
            impl ::froodi::async_impl::Instantiator<#types_with_parenthesis> for #autowired_struct_name<#self_ty>
            {
                type Provides = #self_ty;
                type Error = ::froodi::InstantiateErrorKind;

                #instantiate_function_quote

                #dependencies_function_quote
            }
        }
    } else {
        quote_spanned! { impl_span =>
            impl ::froodi::Instantiator<#types_with_parenthesis> for #autowired_struct_name<#self_ty>
            {
                type Provides = #self_ty;
                type Error = ::froodi::InstantiateErrorKind;

                #instantiate_function_quote

                #dependencies_function_quote
            }
        }
    };
    let global_entry_getter_quote = if is_async {
        quote_spanned! { global_entry_getter_name.span() =>
            #[::froodi::autowired::distributed_slice(::froodi::async_impl::autowired::__GLOBAL_ASYNC_ENTRY_GETTERS)]
            #[linkme(crate = ::froodi::async_impl::autowired::linkme)]
            static #global_entry_getter_name: fn() -> (::froodi::TypeInfo, ::froodi::async_impl::InstantiatorData) = || {
                ::froodi::macros_utils::async_impl::make_entry(
                    #scope,
                    #autowired_struct_name::<#self_ty>(core::marker::PhantomData),
                    #config,
                    #finalizer,
                )
            };
        }
    } else {
        quote_spanned! { global_entry_getter_name.span() =>
            #[::froodi::autowired::distributed_slice(::froodi::autowired::__GLOBAL_ENTRY_GETTERS)]
            #[linkme(crate = ::froodi::autowired::linkme)]
            static #global_entry_getter_name: fn() -> (::froodi::TypeInfo, ::froodi::InstantiatorData) = || {
                ::froodi::macros_utils::sync::make_entry(
                    #scope,
                    #autowired_struct_name::<#self_ty>(core::marker::PhantomData),
                    #config,
                    #finalizer,
                )
            };
        }
    };

    quote_spanned! { impl_span =>
        #[derive(Clone)]
        struct #autowired_struct_name<T>(core::marker::PhantomData<T>);

        #instantiator_impl_quote

        #global_entry_getter_quote
    }
}

pub(crate) fn expand(mut item: Item) -> syn::Result<TokenStream> {
    match item {
        Impl(ItemImpl { trait_: Some(_), .. }) => abort!(item, "you can't use macro with trait"),
        Impl(ItemImpl {
            trait_: None,
            ref mut items,
            ref self_ty,
            ..
        }) => {
            let mut tokens = TokenStream::new();

            let mut provide_attr_found = false;
            for impl_item in items {
                let Fn(ref mut impl_item_fn) = impl_item else {
                    continue;
                };

                let provide_args = match parse_method_attrs(&impl_item_fn.attrs) {
                    Some(Ok(attr)) => attr,
                    Some(Err(err)) => abort!(err.span(), "{err}"),
                    None => continue,
                };

                if provide_attr_found {
                    abort!(impl_item_fn, "#[provide] can only be used once per type");
                }
                provide_attr_found = true;

                // Remove `provide` attribute from final code
                impl_item_fn.attrs.retain(|attr| !attr.path().is_ident("provide"));

                let inst_impl_tokens = expand_instantiator_impl_for_method(&impl_item_fn, self_ty, provide_args);
                tokens.extend(inst_impl_tokens);
            }

            Ok(quote! {
                #item
                #tokens
            })
        }
        _ => abort!(item, "#[injectable] can only be used on `impl` blocks"),
    }
}
