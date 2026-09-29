# Compiled backend architecture

The backend executes through the original `froodi::Container`. Enable
`froodi/compiled` and import `froodi::compiled::{registry, async_registry}`.
The original dynamic macros keep their existing behavior. The experimental
container remains a regression and benchmark reference during integration.

```text
registry! / async_registry!
    ↓
balanced typed registration tree
    ↓
rustc links Inject<T> / InjectTransient<T> to providers
    ↓
bounded const cycle validation at typed container construction
    ↓
materialized RegistrationId edges → explicit final-registry remapping
    ↓
indexed selection → Froodi scope/cache/construction/finalization
```

## Crates and integration seam

| Crate | Responsibility |
|---|---|
| `froodi` with `compiled` | Typed registration adapters and the original container lifecycle |
| `froodi-compile-core` | Shared provider index/linker, const topology, experimental graph IR/compiler; `no_std + alloc` |
| `froodi-compile-macros` | Both frontends' registry syntax to balanced trees; host proc macro |
| `froodi-compile-runtime` | Experimental executor retained for comparison |
| `froodi-compile` | Experimental facade, compatibility scenarios and benchmarks |

Froodi depends on core and macros only. It does not depend on the experimental
runtime. Core's provider witnesses and const DFS are shared, rather than copied.

The seam is `Container::get_selected` / `get_transient_selected`. Public requests
select by type; compiled parameters index the final registration table. Both enter
the **same** lifecycle code. Selection includes the complete `InstantiatorData`:
instantiator, scope, cache policy, dependencies and finalizer. Parent traversal
forwards that selection rather than looking up a provider again.

A compiled `RegistrationExecutor` only needs the native container and its ordered
edge slice to assemble `Deps` and call the original `Instantiator<Deps>`. Its erased
function specializes on `Inst, Deps`; no runtime abstraction or second container
is inserted. Async uses Froodi's existing async container and embedded sync
container, sharing the linker and topology representation.

## Typed linking

`Reg<Out, Inst, Deps, Fin>` retains independently defined functions, captured
closures and `instance(value)` without invoking them. Scope, configuration and
finalization remain registration concerns.

| Abstraction | Role |
|---|---|
| `RegistryIndex` | Projects the value tree to provided types and execution kinds |
| `ProviderPath<T, Path>` | Proves provider identity and declaration-order index |
| `LinkDependency` / `LinkDependencies` | Infers paths and exposes ordered numeric targets |
| `LinkedInject<Path>` / `LinkedInjectTransient<Path>` | Temporary linking witnesses |
| `SupportsExecution` | Rejects a static sync dependency on an async provider |
| `Linked<Out, Inst, Deps, Fin>` | Stores materialized targets, without paths |
| `Collect` / `CollectAsync` | Builds native registration data and owned executors |
| `ResolveDependencies` / `ResolveAsyncDependencies` | Consumes indexed edges in parameter order |

The provider index omits instantiator, dependency and finalizer types. Linking is
shallow even for deep graphs. `Root`, `Path` and `Links` end at linking/validation;
they are absent from executor function signatures. `Link::TOPOLOGY` uses the same
inferred targets, without a second provider search.

Numeric target conversion and runtime registry assembly live in nongeneric
helpers. Otherwise iterator closures inside linking/conversion methods would
retain `Root`/`Links` in their own types and specialize runtime-only work again.

Typed custom parameters use `compiled::Resolver<T>`. They consume no indexed edge
and make topology open. A blanket implementation for every native
`DependencyResolver` would also accept unresolved `Inject<T>` and weaken static
missing/ambiguity checks. Ordinary dynamic registrations accept custom resolvers
without a wrapper. Both injection modes construct immediate dependencies;
`cache_provides` remains independent of injection mode.

## Composition and registration identity

Typed `extend(...)` joins trees before linking. Original runtime registries are
opaque fragments, contribute no provider index, and keep native later-wins
replacement behavior. Declare `runtime::<T>()` or `context::<T>()` to link a typed
parameter crossing such a boundary. These are witnesses, not duplicate runtime
registrations or independent cache policies.

Declaration-order IDs are converted to ordered exact `TypeInfo` keys while the
collection order is known. At container construction, after native merging and
implicit registrations, `prepare` creates the final table and remaps every edge.
Repeated parameters remain repeated; resolvers have no slot. This startup-only
mapping also supports erasing a typed fragment with `IntoRegistry::into_registry`
and composing it later. Erasure defers cycle validation to runtime.

Sync and async retain the native registry namespaces: sync adapters use the sync
table; async adapters use a combined table with native async-first selection.
Each table contains the entire winning registration. Sync registrations remain
sync-only, including inside an async container. A missing opaque target has an
empty table entry: native Context/cache can satisfy normal injection, otherwise
resolution reports `NoInstantiator`. Transient injection bypasses Context/cache.

Runtime cycle and scope validation operates on the effective registry after
replacement. The integration also checks async-to-sync scope reachability.
Opaque resolver/user lookups remain unknown. No blanket validation flag skips
scope/config checks.

## Bounded static cycle validation

The shared const topology flattens numeric adjacency slices and runs iterative
DFS with scratch space for **1,024 total registrations**. The count includes the
implicit container (both native containers in async composition). It checks all
retained declared edges, including unrequested nodes and transient injection.

Typed `Container::new` and `new_with_start_scope` consume `IntoRegistry::VALIDATE`
through `finish`, before witnesses disappear. Creating or extending a fragment,
or explicitly erasing it to a native registry, does not evaluate the check.

| Composition | Cycle guarantee |
|---|---|
| Closed typed tree, including typed fragments, within limit | Const check of all declared edges |
| Runtime fragments/replacements, imports, Context, custom resolvers | Native effective-graph validation; opaque lookups remain unknown |
| Erased or oversized registry | Native runtime validation |

**Evaluation is at monomorphization, not ordinary type checking.** With rustc
1.98.1, `cargo build` and `cargo test` reject instantiated cycle constructors with
E0080. `cargo check`, uninstantiated generic wrappers and unreachable private
functions accept them. Instantiated generic wrappers fail. The integrated
[compilation harness](../../froodi/tests/compiled_diagnostics.rs) verifies these
stages, static missing/ambiguity/sync-to-async failures, an executable diamond with
aliases/captures/instances/reordered fragments, and runtime fallback above the
limit. Its large debug fixture needs a larger test-thread stack.

Arbitrary scope/config values remain runtime values. No guarantee covers arbitrary
lookups made inside an instantiator or resolver.

## Safety contract

The contract is beside the native `RegistrationExecutor` in
[`compiled/mod.rs`](../../froodi/src/compiled/mod.rs), shared by its async adapter:

- Allocate each instantiator in its final `Rc`/`Arc` before deriving a pointer.
  Executors own a shared reference; borrowing async calls cannot outlive it.
- Only private constructors pair a pointer with `construct::<Inst, Deps>`.
  Raw calls remain unsafe. Safe extension points cannot forge this pairing.
  Shared provider-proof traits carry explicit unsafe contracts.
- Remapping preserves parameter order, repeated requests and exact Rust type
  identity. IDs select immutable complete registrations, including finalizers.
- Native cache/transient downcasts are checked. Transient output uses an owned,
  aligned, initialized `Box`; the integrated adapter adds no unchecked output write.
- Finalizers receive native recorded values and the matching winning registration.
  Rc/Arc ownership survives fragment moves, child ownership and executor clones.

Focused Miri suites cover local execution and thread-safe adapters/async calls.
Contended native parking_lot futex calls hit a dependency/Miri blocker, reproduced
with ordinary registries; see [verification limits](compatibility.md#verification-and-limits).
This reviews the changed materialized-edge/ownership boundary, not shutdown,
cancellation or general lifecycle behavior.

## Lifecycle and retirement

Froodi owns scope traversal, cache/Context, construction serialization, close/drop
and finalizer scheduling. Serialization still spans instantiation, finalizer
recording and publication; cache write locks do not span instantiator calls.
Local async construction still serializes even without `thread_safe`.

The experimental runtime's separate lifecycle is retained only as a
test/performance reference; it is not an
integration dependency. Retirement can follow broader compatibility coverage and
a decision on the opt-in frontend. See [compatibility](compatibility.md),
[benchmarks](benchmarks.md), and the [integration ADR](decisions/native-runtime-integration.md).
