# Execute compiled edges through native Froodi lifecycle

Status: accepted for the opt-in `compiled` feature.

Share the provider index/linker and bounded const topology in
`froodi-compile-core`. Generate typed registrations with opt-in native macro
exports; adapt original `Instantiator<Deps>` directly to original containers.
Do not depend on the experimental runtime or introduce a general runtime trait.

Split provider selection from lifecycle handling inside native get/transient.
Compiled parameters select complete registration data through numeric tables;
both dynamic and compiled paths use the existing scope/cache/lock/finalizer code.
Root/path/link witnesses never specialize executor functions.

Convert local declaration IDs to exact type keys while collecting registrations,
then remap to final IDs after runtime composition and implicit registrations.
Retain native replacement and async-first lookup policy. This adds startup work
and keeps type-keyed caches, but removes provider lookup on static edges without
requiring a new runtime storage model.

Evaluate const cycle validation only at typed container construction. Erased
fragments and open compositions use effective runtime validation, so a later
replacement may remove an intermediate cycle. Explicit resolver wrappers prevent
blanket native resolver implementations from weakening static provider errors.

Executors own instantiators in Rc/Arc before pointers are derived. Native checked
output/cache/finalizer conversions remain in place. Experimental unchecked
transient writes and lifecycle code are not imported. Keep the experimental
executor as a regression/performance reference until retirement has equivalent
coverage. Measurements and tested limits live in the architecture, compatibility
and benchmark documents.
