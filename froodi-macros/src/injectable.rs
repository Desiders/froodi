use crate::{
    attr_parsing::parse_attrs,
    injectable::attr::{parse_method_attrs, InstantiatorArgs},
};
use proc_macro2::TokenStream;
use syn::{
    ExprMethodCall,
    ImplItem::Fn,
    ImplItemFn,
    Item::{self, Impl},
    ItemImpl, ItemStruct,
};

mod attr;

pub(crate) fn expand(item: Item) -> syn::Result<TokenStream> {
    match item {
        Impl(ItemImpl { trait_: Some(_), .. }) => return Err(syn::Error::new_spanned(item, "you can't use macro with trait")),
        Impl(ItemImpl {
            attrs,
            defaultness,
            unsafety,
            impl_token,
            generics,
            trait_: None,
            self_ty,
            brace_token,
            items,
        }) => {
            for impl_item in items {
                let Fn(ImplItemFn {
                    attrs,
                    vis,
                    defaultness,
                    sig,
                    block,
                }) = impl_item
                else {
                    let InstantiatorArgs {} = parse_method_attrs(&attrs)?;

                    continue;
                };
            }
        }
        _ => todo!(),
    }

    todo!()
}
