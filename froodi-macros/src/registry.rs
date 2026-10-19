//! Balanced typed registry syntax; provider types are inferred by rustc.

use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use proc_macro2::{Span, TokenStream as TokenStream2, TokenTree};
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::{
    bracketed, parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    Block, Expr, Ident, ItemUse, LitStr, Macro, Token, Type,
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

fn provide_type(input: ParseStream) -> syn::Result<Type> {
    input.parse::<Token![::]>()?;
    input.parse::<Token![<]>()?;
    let ty = input.parse()?;
    input.parse::<Token![>]>()?;
    Ok(ty)
}

/// `provide(entry)` or `provide::<T>()` inside a scope block.
fn parse_scoped_provide(input: ParseStream) -> syn::Result<Registration> {
    let keyword: Ident = input.parse()?;
    if keyword == "provide" && input.peek(Token![::]) {
        let ty = provide_type(input)?;
        let content;
        parenthesized!(content in input);
        return Ok(Registration {
            kind: RegistrationKind::Construct(ty),
            options: parse_options(&content, false)?,
        });
    }
    if keyword != "provide" {
        return Err(syn::Error::new(keyword.span(), "expected `provide(...)` or `provide::<T>()`"));
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
    /// `extend_fragment(name!(args), ...)`
    Fragment(Vec<Macro>),
    /// Expanded fragment body and its lexical argument bindings.
    Group {
        arguments: Block,
        bindings: Block,
        body: RegistryInput,
        name: LitStr,
        identity: LitStr,
    },
    /// The next fragment invocation awaiting expansion.
    Placeholder,
}

impl Parse for Clause {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let keyword: Ident = input.parse()?;
        if keyword == "provide" && input.peek(Token![::]) {
            return Err(syn::Error::new(
                keyword.span(),
                "put `provide::<T>(...)` inside `scope(...) [ ... ]`",
            ));
        }
        let args;
        parenthesized!(args in input);
        if keyword == "extend_fragment" {
            let fragments = Punctuated::<Macro, Token![,]>::parse_terminated(&args)?;
            if fragments.is_empty() {
                return Err(args.error("expected a fragment invocation such as `infrastructure!(config)`"));
            }
            Ok(Self::Fragment(fragments.into_iter().collect()))
        } else if keyword == "__fragment_placeholder" {
            if !args.is_empty() {
                return Err(args.error("unexpected fragment continuation arguments"));
            }
            Ok(Self::Placeholder)
        } else if keyword == "__fragment_group" {
            let arguments = args.parse()?;
            args.parse::<Token![,]>()?;
            let bindings = args.parse()?;
            args.parse::<Token![,]>()?;
            let body;
            bracketed!(body in args);
            args.parse::<Token![,]>()?;
            let name = args.parse()?;
            args.parse::<Token![,]>()?;
            let identity = args.parse()?;
            Ok(Self::Group {
                arguments,
                bindings,
                body: body.parse()?,
                name,
                identity,
            })
        } else if keyword == "scope" {
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
                "expected `scope(...) [ ... ]`, `provide(...)`, `extend(...)` or `extend_fragment(...)`",
            ))
        }
    }
}

pub(crate) struct RegistryInput {
    imports: Vec<ItemUse>,
    clauses: Vec<Clause>,
}

impl Parse for RegistryInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut imports = Vec::new();
        while input.peek(Token![use]) || input.peek(Token![#]) {
            imports.push(input.parse()?);
        }
        Ok(Self {
            imports,
            clauses: input.parse_terminated(Clause::parse, Token![,])?.into_iter().collect(),
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

fn leaf(runtime: &TokenStream2, scope: &Ident, entry: &Registration, origin: &str) -> TokenStream2 {
    let inst = match &entry.kind {
        RegistrationKind::Construct(ty) => {
            let description = quote_spanned!(ty.span()=> ::core::concat!(#origin, "provide::<", ::core::stringify!(#ty), ">() at ", ::core::file!(), ":", ::core::line!(), ":", ::core::column!()));
            return registration_leaf(runtime, scope, entry, quote!(#runtime::provide::<#ty>()), description);
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
    let description = quote_spanned!(inst.span()=> ::core::concat!(#origin, "provide(", #label, ") at ", ::core::file!(), ":", ::core::line!(), ":", ::core::column!()));
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

fn take_fragment(input: &mut RegistryInput) -> syn::Result<Option<Macro>> {
    for index in 0..input.clauses.len() {
        let fragment = match &mut input.clauses[index] {
            Clause::Fragment(fragments) => {
                let fragment = fragments.remove(0);
                if fragments.is_empty() {
                    input.clauses[index] = Clause::Placeholder;
                } else {
                    input.clauses.insert(index, Clause::Placeholder);
                }
                Some(fragment)
            }
            Clause::Group { body, .. } => take_fragment(body)?,
            _ => None,
        };
        if let Some(mut fragment) = fragment {
            if let Some(import) = MacroImports::new(&input.imports).resolve(&mut fragment) {
                let import = &mut input.imports[import];
                if let Some(attribute) = import
                    .attrs
                    .iter()
                    .find(|attribute| attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr"))
                {
                    return Err(syn::Error::new_spanned(
                        attribute,
                        "conditional fragment imports must be placed at module scope, outside `registry!`",
                    ));
                }
                // The import is consumed during staging, before its lexical block exists.
                import.attrs.push(syn::parse_quote!(#[allow(unused_imports)]));
            }
            return Ok(Some(fragment));
        }
    }
    Ok(None)
}

fn fill_placeholder(input: &mut RegistryInput, replacement: &mut Option<Clause>) -> bool {
    for clause in &mut input.clauses {
        match clause {
            Clause::Placeholder => {
                *clause = replacement.take().unwrap();
                return true;
            }
            Clause::Group { body, .. } => {
                if fill_placeholder(body, replacement) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

struct Assembly<'runtime> {
    runtime: &'runtime TokenStream2,
    next: usize,
    first_scopes: Option<Ident>,
    extensions: Vec<(Ident, Ident)>,
    converter: Ident,
    origins: Vec<LitStr>,
}

impl Assembly<'_> {
    fn collect(&mut self, input: &RegistryInput) -> syn::Result<(Vec<TokenStream2>, Vec<TokenStream2>)> {
        let runtime = self.runtime;
        let converter = self.converter.clone();
        let mut bindings = input.imports.iter().map(|import| quote!(#import)).collect::<Vec<_>>();
        let mut leaves = Vec::new();
        for clause in &input.clauses {
            match clause {
                Clause::Group {
                    arguments,
                    bindings: parameters,
                    body,
                    name,
                    ..
                } => {
                    self.origins.push(name.clone());
                    let previous_scopes = self.first_scopes.clone();
                    let previous_extensions = self.extensions.len();
                    let (inner_bindings, inner_leaves) = self.collect(body)?;
                    self.origins.pop();
                    let mut returned = inner_leaves.clone();
                    for (tree, scopes) in &self.extensions[previous_extensions..] {
                        returned.push(quote!(#tree));
                        returned.push(quote!(#scopes));
                    }
                    if previous_scopes.is_none() {
                        if let Some(scopes) = &self.first_scopes {
                            returned.push(quote!(#scopes));
                            returned.push(quote!(#converter));
                        }
                    }
                    // Evaluate arguments outside the import/parameter block, but keep their
                    // temporaries inside the fragment's lexical lifetime.
                    let arguments = &arguments.stmts;
                    let parameters = &parameters.stmts;
                    bindings.push(quote!(let (#(#returned,)*) = {
                        #(#arguments)*
                        { #(#parameters)* #(#inner_bindings)* (#(#returned,)*) }
                    };));
                    leaves.extend(inner_leaves);
                }
                Clause::Extend(registries) => {
                    for registry in registries {
                        let index = self.next;
                        self.next += 1;
                        let tree = format_ident!("__froodi_tree_{index}", span = Span::mixed_site());
                        let scopes = format_ident!("__froodi_extended_scopes_{index}", span = Span::mixed_site());
                        bindings.push(quote!(let (#tree, #scopes) = #runtime::IntoFragment::into_fragment(#registry);));
                        self.extensions.push((tree, scopes));
                    }
                }
                Clause::Scope { .. } | Clause::Provide { .. } => {
                    let (scope, entries) = match clause {
                        Clause::Scope { scope, entries } => (scope, entries.iter().collect::<Vec<_>>()),
                        Clause::Provide { scope, entry } => (scope, vec![&**entry]),
                        _ => unreachable!(),
                    };
                    let index = self.next;
                    self.next += 1;
                    let converted = format_ident!("__froodi_scope_{index}", span = Span::mixed_site());
                    let raw = format_ident!("__froodi_raw_scope_{index}", span = Span::mixed_site());
                    let scope_type = format_ident!("__froodi_scope_type_{index}", span = Span::mixed_site());
                    bindings.push(quote!(let #raw = #scope; let #scope_type = #runtime::ScopeType::new(&#raw);));
                    if self.first_scopes.is_none() {
                        let scopes = format_ident!("__froodi_scopes_{index}", span = Span::mixed_site());
                        bindings.push(quote!(let (#converted, #scopes, #converter) = #runtime::Registry::scope_data(#raw);));
                        self.first_scopes = Some(scopes);
                    } else {
                        bindings.push(quote!(let #converted = #converter.convert(#raw);));
                    }
                    bindings.push(quote!(let #converted = (&&#scope_type).classify(#converted);));
                    let mut origin = String::new();
                    for name in &self.origins {
                        origin.push_str(&name.value());
                        origin.push_str(" -> ");
                    }
                    for entry in entries {
                        let value = leaf(runtime, &converted, entry, &origin);
                        let index = self.next;
                        self.next += 1;
                        let binding = format_ident!("__froodi_registration_{index}", span = Span::mixed_site());
                        bindings.push(quote!(let #binding = #value;));
                        leaves.push(quote!(#binding));
                    }
                }
                Clause::Fragment(_) | Clause::Placeholder => {
                    return Err(syn::Error::new(Span::call_site(), "incomplete registry fragment expansion"));
                }
            }
        }
        Ok((bindings, leaves))
    }
}

pub(crate) fn expand(input: NativeRegistryInput) -> syn::Result<TokenStream2> {
    let NativeRegistryInput { runtime, mut registry } = input;
    if let Some(fragment) = take_fragment(&mut registry)? {
        let path = &fragment.path;
        let arguments = &fragment.tokens;
        return Ok(quote!(#path! { @froodi_fragment (#arguments) [#runtime] [#registry] }));
    }
    let mut assembly = Assembly {
        runtime: &runtime,
        next: 0,
        first_scopes: None,
        extensions: Vec::new(),
        origins: Vec::new(),
        converter: format_ident!("__froodi_scope_converter", span = Span::mixed_site()),
    };
    let (bindings, mut leaves) = assembly.collect(&registry)?;
    let classification = assembly.first_scopes.as_ref().map(|_| quote!(use #runtime::ClassifyScope as _;));
    leaves.extend(assembly.extensions.iter().map(|(tree, _)| quote!(#tree)));
    let tree = balanced(&leaves, &runtime);
    let scopes = assembly
        .first_scopes
        .or_else(|| assembly.extensions.first().map(|(_, scopes)| scopes.clone()));
    let registry = match scopes {
        None => quote!(#runtime::Registry::empty()),
        Some(scopes) => quote!(#runtime::Registry::from_parts(#tree, #scopes)),
    };
    Ok(quote!({ #classification #(#bindings)* #registry }))
}

pub(crate) struct NativeRegistryInput {
    runtime: TokenStream2,
    registry: RegistryInput,
}

impl Parse for NativeRegistryInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut runtime = TokenStream2::new();
        while !input.peek(Token![;]) {
            runtime.extend([input.parse::<TokenTree>()?]);
        }
        input.parse::<Token![;]>()?;
        let registry = if input.peek(Token![@]) {
            input.parse::<Token![@]>()?;
            let resume: Ident = input.parse()?;
            if resume != "resume" {
                return Err(syn::Error::new(resume.span(), "unknown registry continuation"));
            }
            let template;
            bracketed!(template in input);
            let mut registry: RegistryInput = template.parse()?;
            let outer: Block = input.parse()?;
            let bindings: Block = input.parse()?;
            let body;
            bracketed!(body in input);
            let body = body.parse()?;
            let name: LitStr = input.parse()?;
            let identity: LitStr = input.parse()?;
            check_recursion(&registry, &identity, &name, &mut Vec::new())?;
            if !fill_placeholder(
                &mut registry,
                &mut Some(Clause::Group {
                    arguments: outer,
                    bindings,
                    body,
                    name,
                    identity,
                }),
            ) {
                return Err(input.error("fragment continuation is missing"));
            }
            registry
        } else {
            input.parse()?
        };
        Ok(Self { runtime, registry })
    }
}

impl ToTokens for RegistrationOption {
    fn to_tokens(&self, output: &mut TokenStream2) {
        output.extend(match self {
            Self::Config(value) => quote!(config = #value),
            Self::Finalizer(value) => quote!(finalizer = #value),
        });
    }
}

impl ToTokens for Registration {
    fn to_tokens(&self, output: &mut TokenStream2) {
        let options = &self.options;
        output.extend(match &self.kind {
            RegistrationKind::Provide(inst) => quote!(provide(#inst, #(#options,)*)),
            RegistrationKind::Construct(ty) => quote!(provide::<#ty>(#(#options,)*)),
        });
    }
}

impl ToTokens for Clause {
    fn to_tokens(&self, output: &mut TokenStream2) {
        output.extend(match self {
            Clause::Scope { scope, entries } => quote!(scope(#scope) [#(#entries,)*]),
            Clause::Provide { scope, entry } => {
                let RegistrationKind::Provide(inst) = &entry.kind else {
                    unreachable!()
                };
                let options = &entry.options;
                quote!(provide(#scope, #inst, #(#options,)*))
            }
            Clause::Extend(registries) => quote!(extend(#(#registries,)*)),
            Clause::Fragment(fragments) => quote!(extend_fragment(#(#fragments,)*)),
            Clause::Group {
                arguments,
                bindings,
                body,
                name,
                identity,
            } => quote!(__fragment_group(#arguments, #bindings, [#body], #name, #identity)),
            Clause::Placeholder => quote!(__fragment_placeholder()),
        });
    }
}

impl ToTokens for RegistryInput {
    fn to_tokens(&self, output: &mut TokenStream2) {
        let imports = &self.imports;
        let clauses = &self.clauses;
        output.extend(quote!(#(#imports)* #(#clauses,)*));
    }
}

fn check_recursion(input: &RegistryInput, identity: &LitStr, name: &LitStr, ancestors: &mut Vec<(LitStr, LitStr)>) -> syn::Result<bool> {
    for clause in &input.clauses {
        match clause {
            Clause::Placeholder => {
                if ancestors.iter().any(|(ancestor, _)| ancestor.value() == identity.value()) {
                    let mut path = ancestors.iter().map(|(_, name)| name.value()).collect::<Vec<_>>();
                    path.push(name.value());
                    return Err(syn::Error::new(
                        name.span(),
                        format!("recursive registry fragment: {}", path.join(" -> ")),
                    ));
                }
                return Ok(true);
            }
            Clause::Group {
                body,
                identity: ancestor,
                name: origin,
                ..
            } => {
                ancestors.push((ancestor.clone(), origin.clone()));
                if check_recursion(body, identity, name, ancestors)? {
                    return Ok(true);
                }
                ancestors.pop();
            }
            _ => {}
        }
    }
    Ok(false)
}

// Fragment invocations expand before Rust can resolve lexical macro imports.
// Ordinary provider expressions continue to use Rust's normal name resolution.
struct MacroImports {
    paths: Vec<(Ident, syn::Path, usize)>,
}

impl MacroImports {
    fn new(imports: &[ItemUse]) -> Self {
        let mut collected = Self { paths: Vec::new() };
        for (index, import) in imports.iter().enumerate() {
            let prefix = syn::Path {
                leading_colon: import.leading_colon,
                segments: Punctuated::new(),
            };
            collected.collect(&import.tree, &prefix, index);
        }
        collected
    }

    fn collect(&mut self, tree: &syn::UseTree, prefix: &syn::Path, import: usize) {
        let mut path = prefix.clone();
        match tree {
            syn::UseTree::Path(segment) => {
                path.segments.push(syn::PathSegment::from(segment.ident.clone()));
                self.collect(&segment.tree, &path, import);
            }
            syn::UseTree::Name(name) => {
                if name.ident != "self" {
                    path.segments.push(syn::PathSegment::from(name.ident.clone()));
                }
                if let Some(segment) = path.segments.last() {
                    self.paths.push((segment.ident.clone(), path, import));
                }
            }
            syn::UseTree::Rename(rename) => {
                if rename.ident != "self" {
                    path.segments.push(syn::PathSegment::from(rename.ident.clone()));
                }
                self.paths.push((rename.rename.clone(), path, import));
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    self.collect(item, &path, import);
                }
            }
            syn::UseTree::Glob(_) => (),
        }
    }

    fn resolve(&self, invocation: &mut Macro) -> Option<usize> {
        if invocation.path.leading_colon.is_none() {
            if let Some(first) = invocation.path.segments.first() {
                if let Some((_, imported, index)) = self.paths.iter().find(|(name, _, _)| *name == first.ident) {
                    let mut path = imported.clone();
                    path.segments.extend(invocation.path.segments.iter().skip(1).cloned());
                    invocation.path = path;
                    return Some(*index);
                }
            }
        }
        None
    }
}
