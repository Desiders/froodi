# ADR 0001 — Factory model

## Status

Accepted

## Context

Froodi separates factory definition from registration. A factory is an ordinary function or
closure whose parameters are its dependencies; `registry!` attaches scope, `Config` and finalizer
elsewhere. Named functions, inline closures, captured closures and `instance(value)` are all
first-class (requirements §4, §5).

The compile-time engine needs the dependency structure of every factory before runtime, and the
factory value itself at runtime. The question is whether Froodi's `Instantiator<Deps>` already
carries enough static information, or whether factories need a new declaration form.

## Options considered

### Option A — keep `Instantiator<Deps>` and store factory values in typed registrations

The factory's type implements `Instantiator<Deps>`. `Deps` is the tuple of parameter types,
`Provides` the returned value, `Error` the factory error. `registry!` stores the factory value in
a registration whose type records `Provides`, the factory type and `Deps`.

Advantages: no annotation, no second declaration, factories written for Froodi keep compiling.
Disadvantages: the provided type exists only in rustc, never in tokens, so graph checks must go
through trait resolution (ADR 0003).

### Option B — mandatory `#[provide]` annotation on factories

A macro on each factory emits a descriptor with its provided and dependency types.

Advantages: types become visible to macros as tokens.
Disadvantages: factories must know they are factories; closures cannot be annotated; the
descriptor's type names are strings, which are not type identity.

### Option C — erase every factory to `Box<dyn Instantiator>`

Advantages: uniform storage.
Disadvantages: a heap allocation and a dynamic call per factory, and the static dependency
structure is lost at the point of registration.

## Experiments / evidence

`froodi-compile/tests/factory_model.rs` checks what generic code bounded by
`F: Instantiator<Deps>` knows without calling the factory:

| Factory | `Deps` | `Provides` | `Error` | Runtime value |
|---|---|---|---|---|
| named function `make_database(Inject<Config>, InjectTransient<u64>)` | `(Inject<Config>, InjectTransient<u64>)` | `Database` | the declared error | zero-sized function item |
| inline closure `\|Inject(c): Inject<Config>\| ...` | `(Inject<Config>,)` | `Database` | the declared error | zero-sized closure |
| captured closure `move \|\| Ok(url.clone())` | `()` | `String` | the declared error | exactly the captured `String` |
| `instance(value)` | `()` | the value's type | `InstantiateErrorKind` | the value |

The same test shows that two registries differing only in a factory's captured environment
differ in size by exactly that environment, so factories are stored inline, not boxed.

`froodi-compile/tests/compat.rs` runs the four factory kinds against both engines with the same
source.

## Decision

Keep `Instantiator<Deps>` with Froodi's shape (`Provides`, `Error`, `instantiate`), minus the
`dependencies()` method: dependency metadata now comes from the parameter types (`DepMeta`,
`DepsMeta`) and from linking, not from a runtime set.

A registration is a typed leaf `Reg<Provides, Factory, Deps, Finalizer>` holding the factory
value and its registration metadata (scope, `Config`, value source, origin). The position of the
leaf in the registry tree is its factory slot. The slot holds the concrete factory value; nothing
is erased at registration.

## Consequences

### Positive

- Factories stay unannotated and keep being defined apart from registration.
- Captured closures and `instance(value)` are runtime state inside a statically described
  registration.
- A factory call on a static edge is a monomorphized direct call.

### Negative

- Registry types are large and unnameable; they can only be inferred. How fragments cross
  function boundaries is issue #62.
- Parameter types of an inline closure must be annotated, as they must in Froodi today.

### Compatibility impact

`Instantiator<Deps>` loses its `dependencies()` method. It is implemented by the blanket impl for
functions and closures, so user code is affected only if it implements `Instantiator` by hand.

## Rejected alternatives

- Mandatory annotations: they would make factories aware of the DI framework and cannot cover
  closures.
- Boxing every factory: it discards the static structure the engine exists to use and costs an
  allocation and a dynamic call per registration.

## Follow-up work

- Async instantiators on the same leaf type (issue #61).
