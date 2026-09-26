# Fundle Analysis for Froodi

## Purpose

Fundle is relevant because it explores compile-time dependency injection in Rust.

The goal of studying it is **not** to make Froodi adopt Fundle's public API.

The research question is:

> Which Fundle implementation techniques can help compile Froodi's existing programming model?

This document should be refined with exact source references and expansion examples as the dedicated Fundle research work proceeds.

## 1. Evaluation lens

Every Fundle technique should be judged against Froodi requirements.

For each mechanism ask whether it can coexist with:

```text
Instantiator<Deps>
registry!
separate factory definition and registration
captured closures
instance(value)
get<T>()
get_transient<T>()
custom scopes
registry composition
future runtime fallback
```

A technically elegant mechanism that requires Froodi users to adopt a fundamentally different public model should not be copied directly.

## 2. Why Fundle is interesting

The most relevant questions are Rust-specific:

- how dependency relationships are encoded in types or generated code;
- how rustc participates in validating provider/dependency compatibility;
- how static dispatch and monomorphization are obtained;
- how much runtime state remains;
- how macros generate or connect provider code;
- whether a separate global graph compiler is necessary;
- what the cross-module/cross-crate story looks like.

These are implementation questions Froodi also needs to solve.

## 3. Techniques to inspect in Fundle

### Provider representation

Determine how a provider is represented, whether output/dependency information is encoded through traits/associated types, whether providers can hold runtime state, and whether captured values are supported.

Compare this directly to Froodi's `Instantiator<Deps>`.

### Dependency encoding

Determine whether dependency edges exist as explicit graph metadata, type-level relationships, generated calls, trait bounds, or a combination.

The interesting outcome for Froodi is whether rustc can validate actual type relationships while Froodi's own compiler handles graph topology and scope semantics.

### Generated code and monomorphization

Inspect representative generated Rust.

Determine whether Fundle gets performance from direct generic calls, concrete provider types, monomorphized construction, inlining, or dead-code elimination.

This is directly relevant to Froodi's proposed generated typed factory-edge backend.

### Graph validation

Determine how Fundle reports missing providers, ambiguous providers, cycles, and invalid composition.

Especially important:

```text
what is checked by Fundle's own logic
vs
what is rejected indirectly by rustc
```

### Runtime state

Determine what still exists at runtime: provider values, caches, scopes, dynamic dispatch, type lookup, and generated component state.

The useful comparison is not compile time vs runtime as a binary distinction, but **which exact responsibilities remain runtime**.

### Cross-module and cross-crate composition

Froodi wants reusable registry fragments and future cross-crate composition.

Inspect whether Fundle's mechanism scales across crate boundaries and how metadata/traits are transported.

### Diagnostics and build cost

Inspect quality of errors, generated code volume, trait-resolution complexity, clean build impact, incremental build impact, and binary-size implications.

## 4. Techniques that may be useful to Froodi

### Let rustc validate Rust types

A Froodi graph compiler should not attempt to resolve:

```rust
Foo
crate::Foo
module::Foo
Alias
```

as if it were rustc.

A Fundle-like approach may help split responsibilities:

```text
Froodi compiler:
    graph topology
    dependency modes
    scopes
    registration metadata

rustc:
    actual type compatibility
    aliases
    trait bounds
```

### Generated direct dependency calls

If Fundle shows a practical way to compile:

```text
Service -> Repository -> Database
```

into ordinary Rust calls, the same technique may allow Froodi static edges to avoid repeated:

```text
TypeId
map lookup
dyn dispatch
Any/downcast
```

while keeping `container.get::<T>()` at the public boundary.

### Internal type-level structure without public type-level API

Froodi can tolerate complex generated/internal types if users do not have to write or name them.

Fundle may demonstrate useful techniques here.

## 5. Techniques that should not automatically be copied

Even if Fundle uses them successfully, Froodi should be cautious about mechanisms that require:

- provider definitions to live inside special component declarations;
- mandatory provider annotations;
- a public typed component API;
- a closed-world graph with no runtime escape hatch;
- removal of runtime-captured factory state;
- replacing `get/get_transient` with a different programming model;
- merging factory definition and registration.

Any such technique must be adapted rather than adopted verbatim.

## 6. Required research output

The dedicated research should eventually produce a concrete mapping table:

| Fundle technique | What it solves | Equivalent Froodi problem | Preserves Froodi API? | Recommendation |
|---|---|---|---|---|
| TBD | TBD | TBD | TBD | TBD |

For every important mechanism also answer:

```text
Can it work with Instantiator<Deps>?
Can the factory remain declared elsewhere?
Can captured closures work?
Can instance(value) work?
Can registry! remain the composition syntax?
Can get/get_transient remain the public API?
Can registry fragments compose?
```

## 7. Current conclusion

Fundle should influence **how** Froodi compiles dependency relationships and generated execution code.

It should not determine **what users write**.

The highest-value outcome would be a mechanism that lets Froodi keep:

```rust
fn factory(Inject(dep): Inject<Dep>) -> ...
registry! { provide(factory) }
container.get::<T>()
```

while using rustc and generated code to validate and specialize the static portion of the dependency graph.
