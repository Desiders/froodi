//! Runtime dependency graphs must validate on a bounded thread stack.
extern crate std;

use crate::{Dependency, InstantiateErrorKind, TypeInfo};
use alloc::{collections::BTreeSet, vec::Vec};
use std::{ffi::OsStr, process::Command};

const PROBE_ENV: &str = "FROODI_DEPTH_PROBE";
const TEST_NAME: &str = "registry::tests::deep_validation::validates_deep_runtime_graph_on_a_bounded_stack";
const NODE_COUNT: usize = 2048;

struct Node<const INDEX: usize>;

fn node_keys() -> Vec<TypeInfo> {
    // Expand real const-generic TypeIds without compiling a provider per node.
    // Macro nesting is only eleven levels, within the default recursion limit.
    macro_rules! push_keys {
        ($keys:ident, $base:expr;) => {
            $keys.push(TypeInfo::of::<Node<{ $base }>>());
        };
        ($keys:ident, $base:expr; $bit:expr $(, $rest:expr)*) => {{
            push_keys!($keys, $base; $($rest),*);
            push_keys!($keys, ($base) + ($bit); $($rest),*);
        }};
    }
    let mut keys = Vec::with_capacity(NODE_COUNT);
    push_keys!(keys, 0; 1024, 512, 256, 128, 64, 32, 16, 8, 4, 2, 1);
    keys.sort();
    keys
}

#[test]
fn validates_deep_runtime_graph_on_a_bounded_stack() {
    if std::env::var_os(PROBE_ENV).as_deref() != Some(OsStr::new("1")) {
        // A recursive regression aborts on stack overflow. Isolate that abort in
        // a subprocess so it is reported as a normal failure of this one test.
        let output = Command::new(std::env::current_exe().expect("test executable exists"))
            .arg("--exact")
            .arg(TEST_NAME)
            .arg("--nocapture")
            .arg("--test-threads=1")
            .env(PROBE_ENV, "1")
            .current_dir(std::env::temp_dir())
            .output()
            .expect("depth probe subprocess starts");
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "depth probe failed: {}\n{stdout}\n{stderr}", output.status);
        assert!(
            stdout.contains("running 1 test") && stdout.contains("1 passed"),
            "depth probe must execute exactly one test: {stdout}"
        );
        return;
    }

    // Construct descriptors outside the small stack. The worker receives only a
    // Vec of real TypeIds, so it can build its own Rc-backed registry as well.
    let keys = node_keys();
    assert_eq!(keys.len(), NODE_COUNT);
    std::thread::Builder::new()
        .name("deep-registry-validation".into())
        .stack_size(256 * 1024)
        .spawn(move || {
            let mut native = native_registry! {
                provide(crate::DefaultScope::App, || Ok::<_, InstantiateErrorKind>(())),
            };
            // Validation reads metadata only. Sharing one dummy executor avoids
            // thousands of factory monomorphizations; these nodes are not resolved.
            let template = native.entries.remove(&TypeInfo::of::<()>()).unwrap();
            let implicit_entries = native.entries.len();
            for (index, key) in keys.iter().enumerate() {
                let mut entry = template.clone();
                entry.sync_mut().unwrap().metadata.dependencies = keys
                    .get(index + 1)
                    .map_or_else(BTreeSet::new, |next| BTreeSet::from([Dependency { type_info: next.clone() }]));
                native.entries.insert(key.clone(), entry);
            }
            assert_eq!(
                native.entries.len(),
                implicit_entries + NODE_COUNT,
                "all graph IDs must be distinct"
            );
            native.validate().expect("a deep acyclic graph is valid");
        })
        .expect("bounded-stack worker starts")
        .join()
        .expect("bounded-stack validation succeeds");
}
