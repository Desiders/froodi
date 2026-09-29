//! What the analysis reports about each `registry!` invocation.

use std::{collections::BTreeMap, fmt, path::PathBuf};

/// Every `registry!` and `async_registry!` invocation found in one source text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Analysis {
    /// Top-level invocations in source order. Invocations written inside `extend(...)` are
    /// nested in [`Extension::outcome`].
    pub registries: Vec<Registry>,
}

impl Analysis {
    /// Counts registrations and extensions by outcome, nested registries included.
    #[must_use]
    pub fn summary(&self) -> Summary {
        let mut summary = Summary::default();
        for registry in &self.registries {
            summary.add_registry(registry);
        }
        summary
    }

    /// Every registration, nested registries included, in source order.
    #[must_use]
    pub fn registrations(&self) -> Vec<&Registration> {
        let mut out = Vec::new();
        for registry in &self.registries {
            registry.collect_registrations(&mut out);
        }
        out
    }
}

/// The analyses of every `.rs` file under a directory.
#[derive(Debug, Default)]
pub struct DirAnalysis {
    /// Files that parsed, sorted by path.
    pub files: Vec<(PathBuf, Analysis)>,
    /// Files `syn` rejected, or whose `registry!` body is not registry syntax, sorted by path.
    pub unparsed: Vec<(PathBuf, syn::Error)>,
}

impl DirAnalysis {
    /// Counts registrations and extensions by outcome over every parsed file.
    #[must_use]
    pub fn summary(&self) -> Summary {
        let mut summary = Summary::default();
        for (_, analysis) in &self.files {
            for registry in &analysis.registries {
                summary.add_registry(registry);
            }
        }
        summary
    }
}

/// One `registry!` or `async_registry!` invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    /// Which macro was invoked.
    pub kind: RegistryKind,
    /// Line of the macro name, starting at 1.
    pub line: usize,
    /// `provide(...)` entries, both top-level and inside `scope(...) [ ... ]`, in source order.
    pub registrations: Vec<Registration>,
    /// Arguments of `extend(...)`, in source order.
    pub extensions: Vec<Extension>,
}

impl Registry {
    fn collect_registrations<'a>(&'a self, out: &mut Vec<&'a Registration>) {
        out.extend(&self.registrations);
        for extension in &self.extensions {
            if let ExtensionOutcome::Inline(nested) = &extension.outcome {
                nested.collect_registrations(out);
            }
        }
    }

    pub(crate) fn for_each_registration_mut(&mut self, f: &mut impl FnMut(&mut Registration)) {
        self.registrations.iter_mut().for_each(&mut *f);
        for extension in &mut self.extensions {
            if let ExtensionOutcome::Inline(nested) = &mut extension.outcome {
                nested.for_each_registration_mut(f);
            }
        }
    }
}

/// The macro an invocation uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryKind {
    /// `registry! { ... }`
    Sync,
    /// `async_registry! { ... }`
    Async,
}

/// One `provide(...)` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    /// The scope expression as written, for example `Request` or `DefaultScope::App`.
    pub scope: String,
    /// The factory expression as written in the source.
    pub factory: String,
    /// Line of the factory expression, starting at 1.
    pub line: usize,
    /// Whether `config = ...` is given.
    pub has_config: bool,
    /// Whether `finalizer = ...` is given.
    pub has_finalizer: bool,
    /// What the source text says about the provided and dependency types.
    pub outcome: Outcome,
}

/// The classification of one registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The provided type and every dependency are written in the source.
    Resolved {
        /// Spelling of the provided type.
        provides: String,
        /// Dependencies in parameter order.
        deps: Vec<Dependency>,
    },
    /// The source text does not determine the registration's types.
    Unresolved(Reason),
}

/// One factory parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// Spelling of the requested type, the `T` of `Inject<T>` or `InjectTransient<T>`.
    pub ty: String,
    /// How the dependency is requested.
    pub mode: Mode,
}

/// How a factory requests a dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `Inject<T>`: `get` semantics, subject to the provider's cache policy.
    Inject,
    /// `InjectTransient<T>`: a fresh value per request.
    InjectTransient,
}

/// Why the source text does not determine a registration's types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// The factory is a path that does not name a free function declared in the enclosing
    /// module or block of this source: a multi-segment path, an imported name, or an
    /// associated function.
    FactoryNotInFile {
        /// The path as written.
        path: String,
    },
    /// The factory names a `let` binding or a parameter in scope, whose type is inferred.
    FactoryIsALocalValue {
        /// The binding's name.
        name: String,
    },
    /// The factory is an expression other than a path, a closure or `instance(...)`, such as
    /// a call that returns a factory.
    FactoryIsAnExpression,
    /// The factory function has type or const parameters, or `impl Trait` arguments; the
    /// types are fixed by inference at the call site.
    GenericFactory {
        /// The function's name.
        name: String,
    },
    /// A closure parameter has no type annotation.
    ClosureParameterNotAnnotated {
        /// Zero-based parameter index.
        index: usize,
    },
    /// A closure's return type is neither annotated nor spelled by an `Ok::<T, _>(...)` tail.
    ClosureReturnTypeNotWritten,
    /// `instance(value)` where `value` is neither a struct literal nor turbofished.
    InstanceTypeNotWritten,
    /// A parameter type is not `Inject<T>` or `InjectTransient<T>`.
    UnknownDependencyResolver {
        /// Zero-based parameter index.
        index: usize,
        /// The parameter type as written.
        ty: String,
    },
    /// The return type is not `Result<T, _>` or `InstantiatorResult<T>`.
    ReturnTypeIsNotAResult {
        /// The return type as written, empty when none is written.
        ty: String,
    },
    /// A type is written, but the spelling alone does not identify it.
    TypeIsOnlyASpelling {
        /// The provided or dependency type that contains the spelling.
        ty: String,
        /// What makes the spelling ambiguous.
        issue: SpellingIssue,
    },
    /// An `extend(...)` argument is a runtime expression, so its registrations are built by
    /// code the analysis does not run.
    ExtendIsARuntimeExpression,
}

impl Reason {
    /// A stable name of the variant, used as the key of [`Summary::unresolved`].
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::FactoryNotInFile { .. } => "FactoryNotInFile",
            Self::FactoryIsALocalValue { .. } => "FactoryIsALocalValue",
            Self::FactoryIsAnExpression => "FactoryIsAnExpression",
            Self::GenericFactory { .. } => "GenericFactory",
            Self::ClosureParameterNotAnnotated { .. } => "ClosureParameterNotAnnotated",
            Self::ClosureReturnTypeNotWritten => "ClosureReturnTypeNotWritten",
            Self::InstanceTypeNotWritten => "InstanceTypeNotWritten",
            Self::UnknownDependencyResolver { .. } => "UnknownDependencyResolver",
            Self::ReturnTypeIsNotAResult { .. } => "ReturnTypeIsNotAResult",
            Self::TypeIsOnlyASpelling { .. } => "TypeIsOnlyASpelling",
            Self::ExtendIsARuntimeExpression => "ExtendIsARuntimeExpression",
        }
    }
}

/// Why a written type spelling does not identify one type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpellingIssue {
    /// The spelling names a `type` alias declared in the analysed source.
    Alias {
        /// The alias name.
        alias: String,
        /// The aliased type as written.
        target: String,
    },
    /// Several path spellings end in the same name, such as `Config`, `crate::Config` and
    /// `settings::Config`. They may denote one type or several.
    SharedName {
        /// Every spelling that ends in the name, sorted.
        spellings: Vec<String>,
    },
}

/// One argument of `extend(...)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extension {
    /// The argument as written.
    pub expr: String,
    /// Line of the argument, starting at 1.
    pub line: usize,
    /// What the argument contributes.
    pub outcome: ExtensionOutcome,
}

/// What an `extend(...)` argument contributes to the analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionOutcome {
    /// A `registry!` or `async_registry!` written in place, analysed like a top-level one.
    Inline(Registry),
    /// Always [`Reason::ExtendIsARuntimeExpression`].
    Unresolved(Reason),
}

/// Registration counts by outcome.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    /// Registrations whose types are all written.
    pub resolved: usize,
    /// Unresolved registrations and runtime extensions, by [`Reason::name`].
    pub unresolved: BTreeMap<&'static str, usize>,
}

impl Summary {
    /// Resolved plus unresolved items.
    #[must_use]
    pub fn total(&self) -> usize {
        self.resolved + self.unresolved.values().sum::<usize>()
    }

    fn add_registry(&mut self, registry: &Registry) {
        for registration in &registry.registrations {
            match &registration.outcome {
                Outcome::Resolved { .. } => self.resolved += 1,
                Outcome::Unresolved(reason) => *self.unresolved.entry(reason.name()).or_default() += 1,
            }
        }
        for extension in &registry.extensions {
            match &extension.outcome {
                ExtensionOutcome::Inline(nested) => self.add_registry(nested),
                ExtensionOutcome::Unresolved(reason) => *self.unresolved.entry(reason.name()).or_default() += 1,
            }
        }
    }
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "total {}, resolved {}", self.total(), self.resolved)?;
        for (reason, count) in &self.unresolved {
            writeln!(f, "  {reason}: {count}")?;
        }
        Ok(())
    }
}
