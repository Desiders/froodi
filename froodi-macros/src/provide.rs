use alloc::{format, vec::Vec};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
    Data, DeriveInput, Fields, GenericArgument, Ident, PathArguments, Token, Type,
};

use crate::attr_parsing::{parse_attrs, Combine};

#[derive(Clone, Copy)]
enum Mode {
    Inject,
    InjectTransient,
}

struct FieldMode {
    mode: Mode,
    span: Span,
}

impl Combine for FieldMode {
    fn combine(self, other: Self) -> syn::Result<Self> {
        Err(syn::Error::new(
            other.span,
            "dependency mode specified more than once; choose either `inject` or `inject_transient`",
        ))
    }
}

impl Parse for FieldMode {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let mode = if name == "inject" {
            Mode::Inject
        } else if name == "inject_transient" {
            Mode::InjectTransient
        } else {
            return Err(syn::Error::new(
                name.span(),
                "expected `inject` or `inject_transient`; registration options belong in `registry!`",
            ));
        };
        let mut result = Self { mode, span: name.span() };
        if !input.is_empty() {
            if !input.peek(Token![,]) {
                return Err(input.error("dependency modes take no arguments; use `#[di(inject)]` or `#[di(inject_transient)]`"));
            }
            input.parse::<Token![,]>()?;
            if !input.is_empty() {
                result = result.combine(input.parse()?)?;
            }
        }
        Ok(result)
    }
}

fn injected_type(field: &syn::Field) -> syn::Result<Type> {
    let unsupported = || {
        syn::Error::new(
        field.ty.span(),
        "Inject fields require Arc<T>, Rc<T>, or RcThreadSafety<T> with one type argument; select #[di(inject_transient)] for InjectTransient resolution",
    )
    };
    let Type::Path(path) = &field.ty else {
        return Err(unsupported());
    };
    let Some(segment) = path.path.segments.last() else {
        return Err(unsupported());
    };
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Err(unsupported());
    };
    let [GenericArgument::Type(ty)] = args.args.iter().collect::<Vec<_>>()[..] else {
        return Err(unsupported());
    };
    if matches!(ty, Type::TraitObject(_)) {
        return Ok(field.ty.clone());
    }
    Ok(ty.clone())
}

pub(crate) fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let crate_path = match proc_macro_crate::crate_name("froodi") {
        Ok(proc_macro_crate::FoundCrate::Itself) => quote!(crate),
        Ok(proc_macro_crate::FoundCrate::Name(name)) => {
            let name = format_ident!("{name}");
            quote!(::#name)
        }
        Err(_) => return Err(syn::Error::new(input.ident.span(), "cannot find the `froodi` dependency")),
    };
    let name = &input.ident;
    if let Some(attr) = input.attrs.iter().find(|attr| attr.path().is_ident("di")) {
        return Err(syn::Error::new(
            attr.span(),
            "Construct accepts `#[di(inject)]` and `#[di(inject_transient)]` on fields only; registration options belong in `registry!`",
        ));
    }
    let fields = match &input.data {
        Data::Struct(data) => &data.fields,
        _ => return Err(syn::Error::new(input.span(), "Construct can only be derived for structs")),
    };
    if fields.len() > 16 {
        return Err(syn::Error::new(
            fields.span(),
            "Construct supports at most 16 fields, matching Froodi's dependency tuple limit",
        ));
    }
    let mut resolvers = Vec::new();
    let mut bindings = Vec::new();
    let mut values = Vec::new();
    let mut field_bounds = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        let label = field.ident.as_ref().map_or_else(|| format!("{index}"), |ident| format!("{ident}"));
        let context = |err: syn::Error| syn::Error::new(err.span(), format!("Construct field `{name}.{label}`: {err}"));
        let mode = match parse_attrs::<FieldMode>("di", &field.attrs) {
            Some(Ok(mode)) => mode.mode,
            Some(Err((err, _))) => return Err(context(err)),
            None => Mode::Inject,
        };
        let field_type = &field.ty;
        let binding = format_ident!("__froodi_dep_{index}", span = Span::mixed_site());
        let (resolver, value) = match mode {
            Mode::Inject => {
                let dependency = injected_type(field).map_err(context)?;
                field_bounds
                    .push(quote_spanned!(field.ty.span()=> #field_type: #crate_path::macros_utils::typed::ConstructField<#dependency>));
                (
                    quote_spanned!(field.ty.span()=> #crate_path::Inject<#dependency>),
                    quote_spanned!(field.ty.span()=> <#field_type as #crate_path::macros_utils::typed::ConstructField<#dependency>>::from_inject(#binding)),
                )
            }
            Mode::InjectTransient => (
                quote_spanned!(field.ty.span()=> #crate_path::InjectTransient<#field_type>),
                quote_spanned!(field.ty.span()=> #binding.0),
            ),
        };
        resolvers.push(resolver);
        bindings.push(binding);
        values.push(value);
    }
    let assemble = match fields {
        Fields::Named(named) => {
            let names = named.named.iter().map(|field| field.ident.as_ref().unwrap());
            quote!(Self { #(#names: #values,)* })
        }
        Fields::Unnamed(_) => quote!(Self(#(#values,)*)),
        Fields::Unit => quote!(Self),
    };
    let mut generics = input.generics.clone();
    if !generics.params.is_empty() {
        let (_, type_generics, _) = input.generics.split_for_impl();
        let where_clause = generics.make_where_clause();
        where_clause.predicates.push(syn::parse_quote!(#name #type_generics: 'static));
        for resolver in &resolvers {
            where_clause.predicates.push(
                syn::parse_quote!(#resolver: #crate_path::DependencyResolver + #crate_path::macros_utils::typed::SendSafety + 'static),
            );
        }
        for bound in field_bounds {
            where_clause.predicates.push(syn::parse2(bound)?);
        }
    }
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #crate_path::macros_utils::typed::Construct for #name #type_generics #where_clause {
            type Deps = (#(#resolvers,)*);

            fn instantiator() -> impl #crate_path::Instantiator<Self::Deps, Provides = Self, Error = #crate_path::InstantiateErrorKind>
                + #crate_path::macros_utils::typed::SendSafety + #crate_path::macros_utils::typed::SyncSafety
            {
                |#(#bindings: #resolvers),*| ::core::result::Result::Ok::<Self, #crate_path::InstantiateErrorKind>(#assemble)
            }
        }
    })
}
