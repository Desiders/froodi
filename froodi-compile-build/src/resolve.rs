//! Reading the provided and dependency types of one factory from its source text.

use syn::{
    visit::Visit, Block, Expr, ExprCall, ExprClosure, FnArg, GenericArgument, GenericParam, Pat, Path, PathArguments, ReturnType,
    Signature, Stmt, Type, TypeImplTrait,
};

use crate::{
    collect::{Binding, Scopes},
    model::{Dependency, Mode, Outcome, Reason},
    render,
};

pub(crate) fn resolve(factory: &Expr, scopes: &Scopes<'_>) -> Outcome {
    match factory {
        Expr::Path(path) if path.qself.is_none() => named(&path.path, scopes),
        Expr::Closure(closure) => from_closure(closure),
        Expr::Block(block) => match block.block.stmts.last() {
            // `{ let x = x.clone(); move |...| ... }`: the closure is the factory. Names the
            // block declares are out of the analysed scope, so only a closure tail is read.
            Some(Stmt::Expr(Expr::Closure(closure), None)) => from_closure(closure),
            _ => Outcome::Unresolved(Reason::FactoryIsAnExpression),
        },
        Expr::Paren(inner) => resolve(&inner.expr, scopes),
        Expr::Call(call) if is_instance(call) => from_instance(call),
        _ => Outcome::Unresolved(Reason::FactoryIsAnExpression),
    }
}

fn is_instance(call: &ExprCall) -> bool {
    matches!(&*call.func, Expr::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "instance"))
}

/// `instance(value)` provides the type of `value` and has no dependencies. The type is written
/// by `instance::<T>(...)` or by a struct literal with a one-segment path; a longer path may name
/// an enum variant, whose type is the enum.
fn from_instance(call: &ExprCall) -> Outcome {
    let turbofish = match &*call.func {
        Expr::Path(path) => path.path.segments.last().and_then(|segment| match &segment.arguments {
            PathArguments::AngleBracketed(arguments) => match arguments.args.first() {
                Some(GenericArgument::Type(ty)) => Some(render::spelling(ty)),
                _ => None,
            },
            _ => None,
        }),
        _ => None,
    };
    let literal = match call.args.first() {
        Some(Expr::Struct(literal)) if literal.qself.is_none() && literal.path.segments.len() == 1 => Some(render::spelling(&literal.path)),
        _ => None,
    };
    match turbofish.or(literal) {
        Some(provides) => Outcome::Resolved {
            provides,
            deps: Vec::new(),
        },
        None => Outcome::Unresolved(Reason::InstanceTypeNotWritten),
    }
}

/// An inline closure. Parameters must be annotated; the provided type comes from a return
/// annotation, or from an `Ok::<T, _>(...)` tail of the body or of an `async` block it returns.
fn from_closure(closure: &ExprClosure) -> Outcome {
    let parameters = closure.inputs.iter().map(|input| match input {
        Pat::Type(typed) => Some(&*typed.ty),
        _ => None,
    });
    let deps = match dependencies(parameters) {
        Ok(deps) => deps,
        Err(reason) => return Outcome::Unresolved(reason),
    };
    let provides = match &closure.output {
        ReturnType::Type(_, ty) => provided(ty).map_err(|ty| Reason::ReturnTypeIsNotAResult { ty }),
        ReturnType::Default => ok_turbofish(&closure.body)
            .map(render::spelling)
            .ok_or(Reason::ClosureReturnTypeNotWritten),
    };
    match provides {
        Ok(provides) => Outcome::Resolved { provides, deps },
        Err(reason) => Outcome::Unresolved(reason),
    }
}

/// `T` of an `Ok::<T, _>(...)` expression, looking through blocks, `async` blocks and parentheses
/// to their tail expression. `Ok::<_, _>` writes no type.
fn ok_turbofish(expr: &Expr) -> Option<&Type> {
    match expr {
        Expr::Block(block) => block_turbofish(&block.block),
        Expr::Async(block) => block_turbofish(&block.block),
        Expr::Paren(inner) => ok_turbofish(&inner.expr),
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else { return None };
            let segment = path.path.segments.last().filter(|segment| segment.ident == "Ok")?;
            let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                return None;
            };
            match arguments.args.first()? {
                GenericArgument::Type(Type::Infer(_)) => None,
                GenericArgument::Type(ty) => Some(ty),
                _ => None,
            }
        }
        _ => None,
    }
}

fn block_turbofish(block: &Block) -> Option<&Type> {
    match block.stmts.last() {
        Some(Stmt::Expr(expr, None)) => ok_turbofish(expr),
        _ => None,
    }
}

/// A factory written as a path. Only a bare name or `self::name` can name a free function of
/// the enclosing module or block; every other path leads out of the analysed scope.
fn named(path: &Path, scopes: &Scopes<'_>) -> Outcome {
    let not_in_file = || {
        Outcome::Unresolved(Reason::FactoryNotInFile {
            path: render::spelling(path),
        })
    };
    let segments: Vec<_> = path.segments.iter().collect();
    let binding = match (path.leading_colon, segments.as_slice()) {
        (None, [name]) => scopes.lookup(&name.ident.to_string()),
        (None, [module, name]) if module.ident == "self" => scopes.module_function(&name.ident.to_string()).map(Binding::Function),
        _ => None,
    };
    match binding {
        Some(Binding::Function(signature)) => from_signature(signature),
        Some(Binding::Value) => Outcome::Unresolved(Reason::FactoryIsALocalValue {
            name: segments[0].ident.to_string(),
        }),
        None => not_in_file(),
    }
}

/// The types a function signature spells: each parameter must be `Inject<T>` or
/// `InjectTransient<T>`, and the return type `Result<T, _>` or `InstantiatorResult<T>`.
fn from_signature(signature: &Signature) -> Outcome {
    if is_generic(signature) {
        return Outcome::Unresolved(Reason::GenericFactory {
            name: signature.ident.to_string(),
        });
    }
    let parameters = signature.inputs.iter().filter_map(|input| match input {
        FnArg::Typed(typed) => Some(Some(&*typed.ty)),
        FnArg::Receiver(_) => None,
    });
    let output = match &signature.output {
        ReturnType::Type(_, ty) => Some(&**ty),
        ReturnType::Default => None,
    };
    match (dependencies(parameters), output.map_or_else(|| Err(String::new()), provided)) {
        (Err(reason), _) => Outcome::Unresolved(reason),
        (Ok(_), Err(ty)) => Outcome::Unresolved(Reason::ReturnTypeIsNotAResult { ty }),
        (Ok(deps), Ok(provides)) => Outcome::Resolved { provides, deps },
    }
}

fn is_generic(signature: &Signature) -> bool {
    struct ImplTrait(bool);
    impl Visit<'_> for ImplTrait {
        fn visit_type_impl_trait(&mut self, _: &TypeImplTrait) {
            self.0 = true;
        }
    }

    let type_parameters = signature
        .generics
        .params
        .iter()
        .any(|param| matches!(param, GenericParam::Type(_) | GenericParam::Const(_)));
    let mut impl_trait = ImplTrait(false);
    for input in &signature.inputs {
        impl_trait.visit_fn_arg(input);
    }
    type_parameters || impl_trait.0
}

/// The dependencies spelled by parameter types in order. `None` stands for a parameter
/// without a written type.
fn dependencies<'a>(parameters: impl IntoIterator<Item = Option<&'a Type>>) -> Result<Vec<Dependency>, Reason> {
    parameters
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            let ty = ty.ok_or(Reason::ClosureParameterNotAnnotated { index })?;
            dependency(ty).ok_or_else(|| Reason::UnknownDependencyResolver {
                index,
                ty: render::spelling(ty),
            })
        })
        .collect()
}

fn dependency(ty: &Type) -> Option<Dependency> {
    if let Some(inner) = first_type_argument(ty, "Inject") {
        return Some(Dependency {
            ty: render::spelling(inner),
            mode: Mode::Shared,
        });
    }
    first_type_argument(ty, "InjectTransient").map(|inner| Dependency {
        ty: render::spelling(inner),
        mode: Mode::Transient,
    })
}

/// The provided type of a written return type, or the return type's spelling when it is not a
/// `Result`.
fn provided(output: &Type) -> Result<String, String> {
    first_type_argument(output, "Result")
        .or_else(|| first_type_argument(output, "InstantiatorResult"))
        .map(render::spelling)
        .ok_or_else(|| render::spelling(output))
}

/// `T` of a path type whose last segment is `name<T, ...>`.
fn first_type_argument<'a>(ty: &'a Type, name: &str) -> Option<&'a Type> {
    let Type::Path(path) = ty else { return None };
    let last = path.path.segments.last().filter(|segment| segment.ident == name)?;
    let PathArguments::AngleBracketed(arguments) = &last.arguments else {
        return None;
    };
    match arguments.args.first()? {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    }
}
