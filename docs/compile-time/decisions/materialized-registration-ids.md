# End type-level linking at RegistrationId materialization

Status: accepted.

## Evidence

The initial flat-500 app took 12.50 s to build with dependency artifacts warm.
Linking alone took 2.59 s, but linking plus executors took 10.96 s. Rustc's phase
logs showed substantial monomorphization and LLVM IR generation costs after provider
selection. Removing `Root` and `Links` from execution reduced the full build to
4.40 s; independently adding the lightweight provider index reduced it to 3.45 s.
See [measurements and methodology](../benchmarks.md) for the final results.

## Decision

Rustc proves provider existence, uniqueness and execution compatibility during
linking. `ProviderPath` supplies the declaration-order index in the same proof.
`Link` stores the resulting `RegistrationId` requests in a linked registration and
discards the path witnesses from its output type.

Provider lookup uses `RegistryIndex::Index`, a tree of provided types and execution
kinds. The measured reduction justified its associated-type projection cost.
Captured instantiators, instances and finalizers stay in the separate value tree.

Registration collection moves the materialized requests into the IR. Executor
collection operates on linked registrations without a root generic. Erased construction functions specialize only on the
registration type. Dispatch passes the matching compiled edge slice directly to construction, which
consumes it in parameter order; custom resolvers consume no edge. Both sync and async execution retain
indexed table dispatch and existing runtime replacement semantics.

## Consequences

- Missing/ambiguous static providers and sync-to-async dependencies remain compile
  errors. Execution compatibility is checked after provider inference so diagnostics
  identify the actual error.
- Linked leaves hold dependency metadata until it moves into the IR; runtime resolution
  consumes IDs passed from the compiled graph. This trades constant path-derived indices for one
  explicit materialization boundary and smaller executor monomorphizations.
- Graph compilation still validates cycles, scopes and dynamic bindings at container
  construction. This decision changes neither those responsibilities nor public APIs.
- The executor safety contract now includes exact correspondence between the executing
  registration's parameter types and its compiled edges. No registry-root pointer
  crosses the erased boundary.
- The measured local simplifications reach the requested flat-registry build target.
  Fragment interface erasure is deferred; it is not needed to achieve this result.
- Runtime speed is not identical: final deep-resolution medians were roughly 4%
  slower, while cached access was unchanged; the benchmark report includes larger
  differences in other scenarios. The priority of this decision is compiler workload.
