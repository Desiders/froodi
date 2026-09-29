# Integration gaps

These existing differences must be addressed when integrating the compile-time
engine into Froodi. This cleanup does not change their behavior.

| Area | Current limitation and integration impact |
|---|---|
| Duplicate registrations | Froodi permits later registrations to replace earlier ones. Static dependencies instead report ambiguous providers; overrides require explicit `RuntimeRegistry::replacing()` fragments. |
| Context-only dependencies | Static parameters need a `context::<T>()` registration so rustc can link them. Public `get` still accepts unregistered context values. |
| Registry-producing functions | Concrete typed trees are generally impractical to name, and opaque return types hide the provider paths. Use a macro or return `RuntimeRegistry`; the latter validates internal edges at container construction. |
| Custom instantiators | Hand-written implementations use parameter-type metadata instead of Froodi's `dependencies()` method. |
| Closed parent with a live child | Froodi preserves parent cache entries copied when the child was created. The compile-time engine resolves wider values through the parent. Closing the parent early needs compatibility work. |
| Async builders | Async resolution and finalization exist, but the builder/context API does not yet match Froodi's full async API. |
| `froodi-auto` | Static descriptors/adapters and their integration remain to be designed. |

## Further work

- Audit soundness of the erased `RegistrationId` execution table: registration
  pointers, safe extension traits, unchecked edge iteration and downcasts, transient storage,
  finalizers, runtime replacements/imports/context, async cancellation, concurrent
  close and container lifetime. Use the [executor contract](architecture.md#safety-contract)
  as the starting point.
- Investigate graph compiler staging and large-registry type-checking costs without
  changing the selected linking and indexed execution architecture.
- Improve ambiguity diagnostics when Rust's diagnostic facilities allow DI-specific
  explanations.

Registration-level overrides and named cross-crate static fragments are possible
future extensions if concrete usage requires them.
