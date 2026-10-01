# Compiled registrations in Froodi

Enable `froodi/compiled` and use `froodi::compiled_registry!`.
With `async`, use `froodi::compiled_async_registry!`.
Both frontends execute through Froodi's existing containers. The ordinary macros
keep their dynamic semantics, including when a dependency enables `compiled`.

```text
compiled_registry! / compiled_async_registry!
    ↓
balanced typed registration tree
    ↓
rustc proves Inject<T> / InjectTransient<T> → provider
    ↓
bounded const cycle validation at final typed construction
    ↓
RegistrationId edges → final-registry remapping
    ↓
indexed selection → existing scope/cache/construction/finalization
```

## Implementation boundary

| Location | Responsibility |
|---|---|
| `froodi/src/compiled.rs`, `froodi/src/compiled/async_impl.rs` | Private compiled frontend modules |
| `froodi/src/compiled/linking.rs` | Provider index, dependency-link witnesses and typed traversal |
| `froodi/src/compiled/topology.rs` | Const numeric adjacency and bounded cycle validation |
| `froodi/src/compiled/{registry,async_impl/registry}.rs` | Collection, native registry assembly, ID remapping and validation |
| `froodi/src/compiled/{registration,async_impl/registration}.rs` | Registration adapters |
| `froodi/src/{dependency_resolver,inject}.rs` | Dependency resolution, including indexed injection |
| `froodi/src/compiled/dependency_resolver.rs` | Explicit runtime dependency marker |
| `froodi/src/{instantiator,async_impl/instantiator}.rs` | Native instantiator traits and compiled hooks |
| `froodi/src/{instantiator,async_impl/instantiator}/compiled.rs` | Owned erased instantiators |
| `froodi/src/compiled/boundary.rs` | Opaque runtime and Context fragments |
| `froodi/src/compiled/macros.rs` | Hygienic macro wrappers |
| `froodi::macros_utils::compiled` | Hidden public macro expansion helpers |
| `froodi-macros/src/compiled.rs` | Syntax parsing and balanced tree expansion; host only |
| Existing Froodi runtime | Container API, scopes, Context, cache, synchronization and finalizers |

There is no separate compiler-core, facade, build experiment or second runtime.
The shared `froodi-macros` package also provides auto-registration attributes;
its `compiled` feature enables the registry parser. Froodi uses its normal
versioned path dependency policy. Parsing dependencies stay on the host side.

Macros pass a hygienic `$crate::macros_utils::compiled` path through hidden public
helpers. The compiled backend itself is private. This supports
renamed dependencies without downstream imports of implementation crates.
`#[doc(hidden)]` controls documentation, not safety: provider-proof traits retain
explicit unsafe contracts and executor construction remains private.

## Linking and execution

`Registration<Out, Inst, Deps, Fin>` stores independently defined instantiators, captured
closures and `instance(value)`. Finalizers use native `Finalizer<Out>` traits and
are stored as `Option<Fin>`. `RegistryIndex` projects only provider types and
execution kinds. `ProviderPath<T, Path>` proves the declaration-order index;
`LinkDependencies` preserves parameter order and repeated targets.

`LinkedInject<Path>` / `LinkedInjectTransient<Path>` are temporary witnesses.
`Link::TOPOLOGY` uses those same inferred targets, without another provider search.
`Root`, `Path` and `Links` end at linking/validation. `Linked<Out, Inst, Deps, Fin>`
and executor functions retain numeric edges alone. Prototype source-origin and
value-source metadata was never consumed by the native adapters and is removed.

Native sync/async `Instantiator<Deps>` traits supply default `instantiate_compiled`
hooks, resolving `Deps` through the original `DependencyResolver` before invoking
`instantiate`. The resolver's compiled methods default to ordinary resolution for custom
resolvers; tuples preserve edge order, while `Inject` and `InjectTransient` use
indexed overrides. They select complete registration data through `get_selected` /
`get_transient_selected`, then enter the existing lifecycle without repeating
provider lookup by type. Public get, native caches/locks,
custom resolvers and ordinary dynamic instantiators still do type-based work.
`cache_provides` remains independent of injection mode.

## Composition and validation

Typed `extend(...)` joins fragments before linking. Dynamic registries remain
opaque fragments with native later-wins replacement semantics. Typed consumers
use `froodi::runtime::<T>()` or `froodi::context::<T>()` at those boundaries. Custom
parameters use `froodi::RuntimeDependency<T>` to request runtime resolution and
consume no edge; their arbitrary lookups remain opaque.

Collection converts declaration IDs to exact type keys. After composition and
implicit registrations, `prepare` builds the winning registration table and
remaps edges. Scope, cache policy, instantiator and finalizer travel together.
Sync and async retain their native namespaces and async-first selection rules.
Erased fragments retain indexed edges but defer cycle validation to runtime.

Closed typed constructors evaluate `IntoRegistry::VALIDATE` through `finish`.
The const DFS checks all retained immediate edges, including transient and
unrequested registrations, within **1,024 typed declaration leaves**, before native
later-wins assembly. This includes one implicit sync container or both containers
in async composition. Boundary fixtures compose repeated unit leaves: their
declaration count, rather than the smaller final winning table, controls the limit.

On the tested rustc 1.98.1, instantiated `cargo build` / `cargo test` constructors
fail with E0080 for cycles. `cargo check`, uninstantiated wrappers and unused code
accept them. Fragment construction does not validate an intermediate topology.
Open, erased and oversized compositions use effective runtime cycle validation;
opaque lookups cannot be fully checked. Scope/config validation remains runtime
and is never skipped by a general validation flag.

## Executor safety contract

The owning abstraction documents these invariants beside native `ErasedInstantiator`:

- Allocate each Inst in its final Rc/Arc before deriving its pointer; keep its
  owner alive through every call and future.
- Pair the pointer only with its exact `construct::<Inst, Deps>` function.
  Safe extension points cannot manufacture that pairing.
- Preserve edge position, repeats and exact provided Rust types through remapping.
  Custom resolvers consume no indexed position.
- IDs select immutable complete registrations. Imports/replacements select their
  matching scope, cache policy and finalization path.
- Native cached/transient downcasts are checked. Box owns aligned, initialized
  transient output; finalizers receive values from their matching registration.

The prior Miri allocation/provenance finding is addressed by establishing ownership
before pointer collection. Move, child-ownership and aligned-output cases retain
regression coverage. Focused local and thread-safe adapter Miri suites pass after consolidation.
Contended parking_lot futex calls still report unresolved UB in the shared path;
see [compatibility](compatibility.md). This does not establish a complete lifecycle
or shutdown soundness audit. Measurements live in [benchmarks](benchmarks.md).
