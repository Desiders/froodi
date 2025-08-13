use crate::injectable::attr::{parse_method_attrs, InstantiatorArgs};
use proc_macro2::TokenStream;
use syn::{
    ImplItem::Fn,
    ImplItemFn,
    Item::{self, Impl},
    ItemImpl,
};

mod attr;

pub(crate) fn expand(item: Item) -> syn::Result<TokenStream> {
    match item {
        Impl(ItemImpl { trait_: Some(_), .. }) => return Err(syn::Error::new_spanned(item, "you can't use macro with trait")),
        Impl(ItemImpl { trait_: None, items, .. }) => {
            for impl_item in items {
                let Fn(ImplItemFn { attrs, .. }) = impl_item else {
                    continue;
                };

                let InstantiatorArgs {} = match parse_method_attrs(&attrs) {
                    Some(Ok(attr)) => attr,
                    Some(Err(err)) => return Err(err),
                    None => continue,
                };
            }
        }
        _ => todo!(),
    }

    todo!()
}
