# Execute compiled edges through Froodi's lifecycle

Status: accepted. The initial shared-core crate and retained experimental runtime
arrangement is historical and superseded by consolidation.

Provider linking and bounded const topology live in Froodi's private compiled module.
Registry parsing uses the shared `froodi-macros` host package. Adapters
invoke original instantiators through original containers; there is one lifecycle.

Separate selection from lifecycle: compiled parameters select complete
registration data by numeric ID, then use the native scope/cache/lock/finalizer
path. Root/path/link witnesses never specialize executor functions. Convert local
IDs to exact type keys during collection and remap after composition and implicit
registrations, retaining native replacement and async-first selection rules.

Const cycles are evaluated only at final typed construction. Erased/open fragments
use effective runtime validation so replacements can change topology.
`RuntimeDependency<T>` marks custom parameters for runtime resolution, preserving
static provider inference. Scope/config values stay runtime.

Own instantiators in Rc/Arc before deriving executor pointers. Native checked
output/cache/finalizer conversions remain. The prototype container and unchecked
transient implementation are removed. The unresolved shared futex Miri report,
validation limits and measurements remain in the existing documentation.
