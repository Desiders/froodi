# Compiled backend

`registry!` builds a balanced typed registration tree. Rust trait resolution links
`Inject<T>` and `InjectTransient<T>` to registrations; their paths become
`RegistrationId` edges executed through an indexed construction table.

- [Architecture](architecture.md): linking, execution, runtime semantics and safety invariants.
- [Compatibility](compatibility.md): the native opt-in frontend, supported composition and remaining limits.
- [Benchmarks](benchmarks.md): runtime comparisons and build-time measurements.

The `froodi/compiled` feature executes compiled edges through Froodi's original
container lifecycle. Shared linking and bounded const cycle validation live in
`froodi-compile-core`; the experimental runtime remains a regression reference.
Closed typed cycles are checked during instantiated build/test code generation.
Open graphs and scope/config values retain runtime validation.
