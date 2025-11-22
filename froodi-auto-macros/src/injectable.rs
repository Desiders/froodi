mod attr;

use crate::injectable::attr::{parse_method_attrs, ProvideArgs};

use alloc::{boxed::Box, format, string::ToString as _};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned, ToTokens as _};
use syn::{spanned::Spanned as _, Error, Expr, FnArg, Ident, ImplItem, ImplItemFn, Item, ItemImpl, ReturnType, Type};

fn expand_instantiator_impl_for_method(impl_item_fn: &ImplItemFn, self_ty: &Type, provide_args: ProvideArgs) -> syn::Result<TokenStream> {
    let fn_name = &impl_item_fn.sig.ident;
    let is_async = impl_item_fn.sig.asyncness.is_some();
    let impl_span = impl_item_fn.span();

    if matches!(impl_item_fn.sig.output, ReturnType::Default) {
        return Err(Error::new_spanned(
            &impl_item_fn.sig.output,
            "method must have an explicit return type",
        ));
    }

    let types_with_spans = impl_item_fn
        .sig
        .inputs
        .iter()
        .map(|input| match input {
            FnArg::Typed(pat_type) => Ok((pat_type.ty.as_ref(), pat_type.ty.span())),
            FnArg::Receiver(_) => Err(Error::new_spanned(input, "methods with `self` are not supported")),
        })
        .collect::<syn::Result<Box<[_]>>>()?;

    let scope = &provide_args.scope;
    let (config, finalizer) = generate_config_and_finalizer(&provide_args, is_async);

    let (names, names_with_parenthesis, types_with_parenthesis) = generate_parameter_data(&types_with_spans);
    let (autowired_struct_name, global_entry_getter_name) = generate_identifiers(self_ty, fn_name);

    let (instantiate_function_quote, dependencies_function_quote) = generate_function_quotes(
        &types_with_spans,
        self_ty,
        fn_name,
        &names,
        &names_with_parenthesis,
        &types_with_parenthesis,
        impl_span,
        is_async,
    );
    let instantiator_impl_quote = generate_instantiator_impl(
        &autowired_struct_name,
        self_ty,
        &types_with_parenthesis,
        &instantiate_function_quote,
        &dependencies_function_quote,
        impl_span,
        is_async,
    );
    let global_entry_getter_quote = generate_entry_getter(
        &global_entry_getter_name,
        &autowired_struct_name,
        self_ty,
        scope,
        &config,
        &finalizer,
        is_async,
    );

    Ok(quote_spanned! { impl_span =>
        struct #autowired_struct_name<T>(core::marker::PhantomData<T>);

        impl<T> Clone for #autowired_struct_name<T> {
            fn clone(&self) -> Self {
                Self(core::marker::PhantomData)
            }
        }

        #instantiator_impl_quote

        #global_entry_getter_quote
    })
}

fn generate_parameter_data(types_with_spans: &[(&Type, Span)]) -> (TokenStream, TokenStream, TokenStream) {
    if types_with_spans.is_empty() {
        (quote! {}, quote! { () }, quote! { () })
    } else {
        let idents = types_with_spans
            .iter()
            .enumerate()
            .map(|(i, _)| Ident::new(&format!("arg{i}"), Span::call_site()))
            .collect::<Box<[_]>>();

        let names = quote! { #( #idents ),* };
        let names_with_parenthesis = quote! { ( #( #idents ),* ) };

        let types_with_parenthesis = {
            let quoted_types = types_with_spans.iter().map(|(ty, span)| {
                quote_spanned! { *span => #ty }
            });
            quote! { ( #( #quoted_types ),* ) }
        };

        (names, names_with_parenthesis, types_with_parenthesis)
    }
}

fn generate_config_and_finalizer(provide_args: &ProvideArgs, is_async: bool) -> (TokenStream, TokenStream) {
    let config = match &provide_args.config {
        Some((_, config)) => quote_spanned! { config.span() => Some(#config) },
        None => quote! { None },
    };
    let finalizer = match &provide_args.finalizer {
        Some((_, finalizer)) => quote_spanned! { finalizer.span() => Some(#finalizer) },
        None => {
            if is_async {
                quote! { None::<::froodi::macros_utils::async_impl::FinDummy<_>> }
            } else {
                quote! { None::<::froodi::macros_utils::sync::FinDummy<_>> }
            }
        }
    };
    (config, finalizer)
}

fn generate_identifiers(self_ty: &Type, fn_name: &Ident) -> (Ident, Ident) {
    let self_ty_uppercase = self_ty.into_token_stream().to_string().to_uppercase();
    let fn_name_uppercase = fn_name.to_string().to_uppercase();

    let autowired_struct_name = format_ident!("__Autowired{}_{}", self_ty_uppercase, fn_name_uppercase);
    let global_entry_getter_name = format_ident!("__{}_{}", self_ty_uppercase, fn_name_uppercase);

    (autowired_struct_name, global_entry_getter_name)
}

fn generate_function_quotes(
    types_with_spans: &[(&Type, Span)],
    self_ty: &Type,
    fn_name: &Ident,
    names: &TokenStream,
    names_with_parenthesis: &TokenStream,
    types_with_parenthesis: &TokenStream,
    impl_span: Span,
    is_async: bool,
) -> (TokenStream, TokenStream) {
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

    (instantiate_function_quote, dependencies_function_quote)
}

fn generate_instantiator_impl(
    autowired_struct_name: &Ident,
    self_ty: &Type,
    types_with_parenthesis: &TokenStream,
    instantiate_function_quote: &TokenStream,
    dependencies_function_quote: &TokenStream,
    impl_span: Span,
    is_async: bool,
) -> TokenStream {
    if is_async {
        quote_spanned! { impl_span =>
            impl ::froodi::async_impl::Instantiator<#types_with_parenthesis> for #autowired_struct_name<#self_ty> {
                type Provides = #self_ty;
                type Error = ::froodi::InstantiateErrorKind;

                #instantiate_function_quote
                #dependencies_function_quote
            }
        }
    } else {
        quote_spanned! { impl_span =>
            impl ::froodi::Instantiator<#types_with_parenthesis> for #autowired_struct_name<#self_ty> {
                type Provides = #self_ty;
                type Error = ::froodi::InstantiateErrorKind;

                #instantiate_function_quote
                #dependencies_function_quote
            }
        }
    }
}

fn generate_entry_getter(
    global_entry_getter_name: &Ident,
    autowired_struct_name: &Ident,
    self_ty: &Type,
    scope: &Expr,
    config: &TokenStream,
    finalizer: &TokenStream,
    is_async: bool,
) -> TokenStream {
    if is_async {
        quote_spanned! { global_entry_getter_name.span() =>
            #[::froodi_auto::entry_getters::distributed_slice(::froodi_auto::entry_getters::__ASYNC_ENTRY_GETTERS)]
            #[linkme(crate = ::froodi_auto::entry_getters::linkme)]
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
            #[::froodi_auto::entry_getters::distributed_slice(::froodi_auto::entry_getters::__ENTRY_GETTERS)]
            #[linkme(crate = ::froodi_auto::entry_getters::linkme)]
            static #global_entry_getter_name: fn() -> (::froodi::TypeInfo, ::froodi::InstantiatorData) = || {
                ::froodi::macros_utils::sync::make_entry(
                    #scope,
                    #autowired_struct_name::<#self_ty>(core::marker::PhantomData),
                    #config,
                    #finalizer,
                )
            };
        }
    }
}

pub(crate) fn expand(mut item: Item) -> syn::Result<TokenStream> {
    match item {
        Item::Impl(ItemImpl { trait_: Some(_), .. }) => Err(Error::new_spanned(item, "you can't use macro with trait")),
        Item::Impl(ItemImpl {
            trait_: None,
            ref mut items,
            ref self_ty,
            ..
        }) => {
            let mut provide_methods = items
                .into_iter()
                .filter_map(|item| {
                    if let ImplItem::Fn(impl_item_fn) = item {
                        Some(impl_item_fn)
                    } else {
                        None
                    }
                })
                .filter(|impl_item_fn| parse_method_attrs(&impl_item_fn.attrs).is_some())
                .collect::<Box<[_]>>();

            if provide_methods.is_empty() {
                return Ok(quote! { #item });
            }
            if provide_methods.len() > 1 {
                return Err(Error::new_spanned(&provide_methods[1], "#[provide] can only be used once per type"));
            }

            let impl_item_fn = &mut provide_methods[0];
            let provide_args = match parse_method_attrs(&impl_item_fn.attrs).expect("provide method exists") {
                Ok(val) => val,
                Err(err) => return Err(Error::new_spanned(impl_item_fn, err)),
            };

            // Remove `provide` attribute from final code
            impl_item_fn.attrs.retain(|attr| !attr.path().is_ident("provide"));

            let tokens = expand_instantiator_impl_for_method(impl_item_fn, self_ty, provide_args)?;

            Ok(quote! {
                #item
                #tokens
            })
        }
        _ => Err(Error::new_spanned(item, "#[injectable] can only be used on `impl` blocks")),
    }
}
