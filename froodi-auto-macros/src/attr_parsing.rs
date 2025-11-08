use core::any;
use quote::ToTokens;
use syn::{
    parse::{Parse, ParseStream},
    Token,
};

pub(crate) fn parse_assignment_attribute<K, T>(input: ParseStream<'_>, out: &mut Option<(K, T)>) -> syn::Result<()>
where
    K: Parse + ToTokens,
    T: Parse,
{
    let kw = input.parse()?;
    input.parse::<Token![=]>()?;
    let inner = input.parse()?;

    if out.is_some() {
        let kw_name = any::type_name::<K>().split("::").last().unwrap();
        let msg = "` specified more than once";
        return Err(syn::Error::new_spanned(kw, &["`", kw_name, msg].concat()));
    }

    *out = Some((kw, inner));

    Ok(())
}

pub(crate) trait Combine: Sized {
    fn combine(self, other: Self) -> syn::Result<Self>;
}

pub(crate) fn parse_attrs<T>(ident: &str, attrs: &[syn::Attribute]) -> Option<Result<T, (syn::Error, syn::Attribute)>>
where
    T: Combine + Parse,
{
    let mut iter = attrs
        .iter()
        .filter(|attr| attr.meta.path().is_ident(ident))
        .map(|attr| (attr, attr.parse_args::<T>()));

    let first = match iter.next() {
        Some((_, Ok(first))) => first,
        Some((attr, Err(err))) => return Some(Err((err, attr.clone()))),
        None => return None,
    };

    let result = iter.try_fold(first, |out, (attr, next_result)| match next_result {
        Ok(next) => out.combine(next).map_err(|err| (err, attr.clone())),
        Err(err) => Err((err, attr.clone())),
    });

    Some(result)
}

pub(crate) fn combine_attribute<K, T>(a: &mut Option<(K, T)>, b: Option<(K, T)>) -> syn::Result<()>
where
    K: ToTokens,
{
    if let Some((kw, inner)) = b {
        if a.is_some() {
            let kw_name = any::type_name::<K>().split("::").last().unwrap();
            let msg = "` specified more than once";
            return Err(syn::Error::new_spanned(kw, &["`", kw_name, msg].concat()));
        }
        *a = Some((kw, inner));
    }
    Ok(())
}
