//! Walks a parsed file, tracks which names are in scope, and builds a [`Registry`] for every
//! `registry!` and `async_registry!` invocation.

use std::collections::{BTreeMap, HashMap, HashSet};

use syn::{
    spanned::Spanned,
    visit::{self, Visit},
    Block, Expr, ExprClosure, File, ImplItemFn, Item, ItemFn, ItemMod, ItemType, Local, Macro, Pat, PatIdent, Signature, Stmt,
};

use crate::{
    model::{Extension, ExtensionOutcome, Reason, Registration, Registry, RegistryKind},
    render,
    resolve::resolve,
    syntax::{Clause, Entry, RegistryBody},
};

/// What a single-segment name refers to at an invocation site.
pub(crate) enum Binding<'ast> {
    /// A free function item.
    Function(&'ast Signature),
    /// A `let` binding or a parameter.
    Value,
}

/// Names declared by one module, block, function or closure.
#[derive(Default)]
struct Frame<'ast> {
    functions: HashMap<String, &'ast Signature>,
    values: HashSet<String>,
}

impl<'ast> Frame<'ast> {
    fn of_items(items: impl IntoIterator<Item = &'ast Item>) -> Self {
        let functions = items
            .into_iter()
            .filter_map(|item| match item {
                Item::Fn(function) => Some((function.sig.ident.to_string(), &function.sig)),
                _ => None,
            })
            .collect();
        Self {
            functions,
            values: HashSet::new(),
        }
    }

    fn of_patterns<'p>(patterns: impl IntoIterator<Item = &'p Pat>) -> Self {
        let mut frame = Self::default();
        for pattern in patterns {
            frame.bind(pattern);
        }
        frame
    }

    fn bind(&mut self, pattern: &Pat) {
        struct Names<'f>(&'f mut HashSet<String>);

        impl Visit<'_> for Names<'_> {
            fn visit_pat_ident(&mut self, ident: &PatIdent) {
                self.0.insert(ident.ident.to_string());
                visit::visit_pat_ident(self, ident);
            }
        }
        Names(&mut self.values).visit_pat(pattern);
    }
}

/// The names visible at one point of the file, innermost frame last. A module starts a fresh
/// stack because items of an enclosing module are not in scope inside it.
#[derive(Default)]
pub(crate) struct Scopes<'ast> {
    frames: Vec<Frame<'ast>>,
}

impl<'ast> Scopes<'ast> {
    /// What `name` refers to, looking from the innermost frame outwards.
    pub(crate) fn lookup(&self, name: &str) -> Option<Binding<'ast>> {
        self.frames.iter().rev().find_map(|frame| {
            if frame.values.contains(name) {
                Some(Binding::Value)
            } else {
                frame.functions.get(name).map(|signature| Binding::Function(signature))
            }
        })
    }

    /// The free function `name` declared by the enclosing module itself, for `self::name`.
    pub(crate) fn module_function(&self, name: &str) -> Option<&'ast Signature> {
        self.frames.first().and_then(|frame| frame.functions.get(name).copied())
    }
}

/// Every invocation of the file and every `type` alias declared in it.
pub(crate) struct Collected {
    pub(crate) registries: Vec<Registry>,
    pub(crate) aliases: BTreeMap<String, String>,
}

pub(crate) fn collect(file: &File) -> syn::Result<Collected> {
    let mut collector = Collector {
        scopes: Scopes::default(),
        registries: Vec::new(),
        aliases: BTreeMap::new(),
        error: None,
    };
    collector.visit_file(file);
    match collector.error {
        Some(error) => Err(error),
        None => Ok(Collected {
            registries: collector.registries,
            aliases: collector.aliases,
        }),
    }
}

struct Collector<'ast> {
    scopes: Scopes<'ast>,
    registries: Vec<Registry>,
    aliases: BTreeMap<String, String>,
    error: Option<syn::Error>,
}

impl<'ast> Collector<'ast> {
    fn within(&mut self, frame: Frame<'ast>, walk: impl FnOnce(&mut Self)) {
        self.scopes.frames.push(frame);
        walk(self);
        self.scopes.frames.pop();
    }
}

impl<'ast> Visit<'ast> for Collector<'ast> {
    fn visit_file(&mut self, file: &'ast File) {
        self.within(Frame::of_items(&file.items), |this| visit::visit_file(this, file));
    }

    fn visit_item_mod(&mut self, module: &'ast ItemMod) {
        let Some((_, items)) = &module.content else { return };
        let outer = std::mem::take(&mut self.scopes);
        self.within(Frame::of_items(items), |this| visit::visit_item_mod(this, module));
        self.scopes = outer;
    }

    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        let parameters = function.sig.inputs.iter().filter_map(|input| match input {
            syn::FnArg::Typed(typed) => Some(&*typed.pat),
            syn::FnArg::Receiver(_) => None,
        });
        self.within(Frame::of_patterns(parameters), |this| visit::visit_item_fn(this, function));
    }

    fn visit_impl_item_fn(&mut self, function: &'ast ImplItemFn) {
        let parameters = function.sig.inputs.iter().filter_map(|input| match input {
            syn::FnArg::Typed(typed) => Some(&*typed.pat),
            syn::FnArg::Receiver(_) => None,
        });
        self.within(Frame::of_patterns(parameters), |this| visit::visit_impl_item_fn(this, function));
    }

    fn visit_expr_closure(&mut self, closure: &'ast ExprClosure) {
        self.within(Frame::of_patterns(&closure.inputs), |this| visit::visit_expr_closure(this, closure));
    }

    fn visit_block(&mut self, block: &'ast Block) {
        let items = block.stmts.iter().filter_map(|stmt| match stmt {
            Stmt::Item(item) => Some(item),
            _ => None,
        });
        self.within(Frame::of_items(items), |this| {
            for stmt in &block.stmts {
                this.visit_stmt(stmt);
            }
        });
    }

    fn visit_local(&mut self, local: &'ast Local) {
        // The initializer does not see the binding it initializes.
        if let Some(init) = &local.init {
            self.visit_local_init(init);
        }
        if let Some(frame) = self.scopes.frames.last_mut() {
            frame.bind(&local.pat);
        }
    }

    fn visit_item_type(&mut self, alias: &'ast ItemType) {
        self.aliases.insert(alias.ident.to_string(), render::spelling(&alias.ty));
        visit::visit_item_type(self, alias);
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        if self.error.is_some() {
            return;
        }
        let Some(kind) = registry_kind(mac) else { return };
        match mac.parse_body::<RegistryBody>() {
            Ok(body) => match build(kind, mac, body, &self.scopes) {
                Ok(registry) => self.registries.push(registry),
                Err(error) => self.error = Some(error),
            },
            Err(error) => self.error = Some(error),
        }
    }
}

fn registry_kind(mac: &Macro) -> Option<RegistryKind> {
    let name = &mac.path.segments.last()?.ident;
    if name == "registry" {
        Some(RegistryKind::Sync)
    } else if name == "async_registry" {
        Some(RegistryKind::Async)
    } else {
        None
    }
}

fn build(kind: RegistryKind, mac: &Macro, body: RegistryBody, scopes: &Scopes<'_>) -> syn::Result<Registry> {
    let mut registry = Registry {
        kind,
        line: mac.path.span().start().line,
        registrations: Vec::new(),
        extensions: Vec::new(),
    };
    for clause in body.clauses {
        match clause {
            Clause::Scope { scope, entries } => {
                for entry in &entries {
                    registry.registrations.push(registration(&scope, entry, scopes));
                }
            }
            Clause::Provide { scope, entry } => registry.registrations.push(registration(&scope, &entry, scopes)),
            Clause::Extend(arguments) => {
                for argument in &arguments {
                    registry.extensions.push(extension(argument, scopes)?);
                }
            }
        }
    }
    Ok(registry)
}

fn registration(scope: &Expr, entry: &Entry, scopes: &Scopes<'_>) -> Registration {
    Registration {
        scope: render::spelling(scope),
        factory: render::source(&entry.factory, entry.factory.span()),
        line: entry.factory.span().start().line,
        has_config: entry.config.is_some(),
        has_finalizer: entry.finalizer.is_some(),
        outcome: resolve(&entry.factory, scopes),
    }
}

/// An `extend(...)` argument. A `registry!` written in place is analysed like a top-level one;
/// any other expression builds its registry at runtime.
fn extension(argument: &Expr, scopes: &Scopes<'_>) -> syn::Result<Extension> {
    let nested = match argument {
        Expr::Macro(expr) => registry_kind(&expr.mac).map(|kind| (kind, &expr.mac)),
        _ => None,
    };
    let outcome = match nested {
        Some((kind, mac)) => ExtensionOutcome::Inline(build(kind, mac, mac.parse_body()?, scopes)?),
        None => ExtensionOutcome::Unresolved(Reason::ExtendIsARuntimeExpression),
    };
    Ok(Extension {
        expr: render::source(argument, argument.span()),
        line: argument.span().start().line,
        outcome,
    })
}
