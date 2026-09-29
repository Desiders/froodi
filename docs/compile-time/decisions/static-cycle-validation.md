# Const cycle validation before witness erasure

Status: accepted as a bounded experiment.

Reuse inferred `Link<Root, Links>` witnesses to expose const numeric adjacency
slices. Validate them with iterative DFS when the final container constructor is
monomorphized, then discard witnesses and retain the existing indexed runtime.
No recursive constructibility traits, second type-linking pass or new user
annotations are needed.

The check covers closed static compositions up to 1,024 nodes, including the
implicit container. Runtime fragments/replacements, imports, context declarations
and custom resolvers make the composition open; larger/open graphs keep runtime
validation. This prevents rejection of topology that a later replacement changes.
Scope/config values remain runtime-validated.

`Plan::build_with` must evaluate the associated validation constant. On rustc
1.98.1, instantiated sync/async constructors and generic wrappers fail during
`cargo build`/`cargo test`; `cargo check` and uninstantiated functions do not force
evaluation. The compilation-stage fixtures preserve this limitation explicitly.
There is no claim of full static validation of arbitrary user lookups.

The extra const work is measured independently by the `linking` and `validated`
benchmark stages; full before/after results are in [benchmarks](../benchmarks.md).
Runtime executors remain specialized only for their concrete registration type.
