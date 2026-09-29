# Compile-time engine

`registry!` builds a balanced typed registration tree. Rust trait resolution links
`Inject<T>` and `InjectTransient<T>` to registrations; their paths become
`RegistrationId` edges executed through an indexed construction table.

- [Architecture](architecture.md): linking, execution, runtime semantics and safety invariants.
- [Integration gaps](compatibility.md): differences to resolve before integrating with Froodi.
- [Benchmarks](benchmarks.md): runtime comparisons and build-time measurements.

The engine lives in the `froodi-compile*` crates alongside Froodi. Cycle and scope
validation still run when the container is built. Future graph compiler work must
preserve Froodi's dependency, cache, scope and lifecycle semantics.
