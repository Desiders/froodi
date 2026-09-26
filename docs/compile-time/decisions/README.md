# Architecture Decision Records

Use this directory for decisions that have been validated by source analysis and/or experiments.

Do not use ADRs merely to record speculative ideas.

## Naming

```text
0001-short-decision-name.md
0002-another-decision.md
...
```

## Suggested early ADRs

```text
0001-factory-model.md
0002-registry-compilation-strategy.md
0003-type-identity-strategy.md
0004-static-execution-backend.md
0005-registry-fragment-composition.md
0006-static-runtime-boundary.md
```

Create them only after the relevant experiments are complete.

## Template

```markdown
# ADR NNNN — Title

## Status

Proposed | Accepted | Superseded | Rejected

## Context

What Froodi behavior/problem requires a decision?
What constraints from `../requirements.md` apply?

## Options considered

### Option A

Description.
Advantages.
Disadvantages.

### Option B

Description.
Advantages.
Disadvantages.

## Experiments / evidence

What code was prototyped?
What benchmarks or source findings support the decision?

## Decision

What are we choosing?

## Consequences

### Positive
...

### Negative
...

### Compatibility impact

Does this change any Froodi user-facing syntax or semantics?
If yes, why is that unavoidable?

## Rejected alternatives

Why were they rejected?

## Follow-up work
...
```

## ADR policy

A decision that breaks or significantly changes existing Froodi behavior must explicitly document:

```text
existing behavior
reason exact preservation failed
alternatives tested
minimal required API difference
future integration impact
```
