# Compile-Time Froodi

This directory contains the design and research documentation for the experimental compile-time/static Froodi engine.

The implementation is being developed in **new experimental crates** so the compile-time architecture can be explored without simultaneously solving backward-compatible integration with the current runtime engine.

The long-term destination is still **Froodi itself**. This is not a separate DI framework.

## Current direction

```text
existing Froodi programming model
        ↓
registry!/async_registry! compatible frontend
        ↓
compile-time graph analysis
        ↓
compiled graph / generated execution metadata
        ↓
Froodi-like Container
        ↓
get<T>() / get_transient<T>()
```

The implementation should preserve Froodi semantics aggressively while moving structurally known work out of runtime.

The most important architectural distinction is:

```text
compile-time dependency topology
+
runtime factory/application state
```

A factory can therefore remain an ordinary Rust function or closure, including a captured closure, while its dependency structure is compiled ahead of runtime where possible.

## Documents

- [requirements.md](requirements.md) — project invariants and non-negotiable compatibility constraints.
- [froodi-analysis.md](froodi-analysis.md) — current Froodi programming model and runtime responsibilities relevant to the new engine.
- [fundle-analysis.md](fundle-analysis.md) — Fundle techniques relevant to Froodi and the constraints on adopting them.
- [architecture.md](architecture.md) — current working architecture and unresolved design choices.
- [compatibility.md](compatibility.md) — feature/API compatibility matrix between current Froodi and the experimental engine.
- [benchmarks.md](benchmarks.md) — benchmark strategy and required measurements.
- [decisions/](decisions/) — Architecture Decision Records for decisions that have been experimentally validated.

## Source of truth

Architectural evidence should be prioritized in this order:

```text
1. Existing Froodi implementation, tests and examples
2. Fundle implementation
3. Other Rust compile-time DI implementations
4. Dagger / Spring AOT / other ecosystems
```

External DI systems are implementation references. They must not redefine Froodi's public programming model.

## Development isolation

The experimental implementation lives in new crates:

```text
froodi-compile
froodi-compile-core
froodi-compile-runtime
froodi-compile-macros
froodi-compile-build
```

[architecture.md](architecture.md) describes what each crate holds.

The important boundary is:

```text
existing Froodi implementation
    !=
experimental compile-time implementation
```

The two engines share no code. `froodi-compile/tests/compat.rs` compiles the same scenarios
against both to catch API drift.

## Documentation policy

GitHub issues describe work to perform. The documents in this directory describe what has been learned, selected, or constrained.

```text
GitHub Issue
    = what we need to investigate or implement

docs/compile-time/*
    = what we currently know and what we decided

code
    = implementation of those decisions
```

When a design choice becomes stable, record it as an ADR in `decisions/` rather than leaving the rationale only in an issue or pull request discussion.
