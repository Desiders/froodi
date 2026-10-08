//! Const topology exists only during linking; executors retain numeric edges alone.

use crate::ScopeData;
use core::str::from_utf8;

const LIMIT: usize = 1024;

pub struct Topology {
    count: usize,
    closed: bool,
    fixed: bool,
    has_static_scopes: bool,
    shape: Shape,
}

enum Shape {
    Empty,
    Leaf(TopologyLeaf),
    Branch(&'static Topology, &'static Topology),
}

#[derive(Clone, Copy)]
struct TopologyLeaf {
    targets: &'static [Option<usize>],
    source: &'static str,
    scope: Option<&'static ScopeData>,
}

impl TopologyLeaf {
    const EMPTY: Self = Self {
        targets: &[],
        source: "unnamed registration",
        scope: None,
    };
}

impl Topology {
    pub const EMPTY: Self = Self {
        count: 0,
        closed: true,
        fixed: true,
        has_static_scopes: false,
        shape: Shape::Empty,
    };
    pub const OPEN: Self = Self {
        count: 0,
        closed: false,
        fixed: false,
        has_static_scopes: false,
        shape: Shape::Empty,
    };

    pub const fn leaf(targets: &'static [Option<usize>]) -> Self {
        let mut closed = true;
        let mut index = 0;
        while index < targets.len() {
            if targets[index].is_none() {
                closed = false;
            }
            index += 1;
        }
        Self {
            count: 1,
            closed,
            fixed: true,
            has_static_scopes: false,
            shape: Shape::Leaf(TopologyLeaf {
                targets,
                ..TopologyLeaf::EMPTY
            }),
        }
    }

    pub const fn with_source(mut self, source: &'static str) -> Self {
        if let Shape::Leaf(ref mut leaf) = self.shape {
            leaf.source = source;
        }
        self
    }

    pub const fn with_scope(mut self, scope: Option<&'static ScopeData>) -> Self {
        if let Shape::Leaf(ref mut leaf) = self.shape {
            leaf.scope = scope;
            self.has_static_scopes = scope.is_some();
        }
        self
    }

    pub const fn branch(left: &'static Self, right: &'static Self) -> Self {
        Self {
            count: left.count + right.count,
            closed: left.closed && right.closed,
            fixed: left.fixed && right.fixed,
            has_static_scopes: left.has_static_scopes || right.has_static_scopes,
            shape: Shape::Branch(left, right),
        }
    }

    const fn flatten(&self, nodes: &mut [TopologyLeaf; LIMIT], offset: usize) {
        match self.shape {
            Shape::Empty => (),
            Shape::Leaf(leaf) => nodes[offset] = leaf,
            Shape::Branch(left, right) => {
                left.flatten(nodes, offset);
                right.flatten(nodes, offset + left.count);
            }
        }
    }

    pub const fn can_validate_all_cycles(&self) -> bool {
        self.closed && self.count <= LIMIT
    }

    pub const fn has_fixed_ids(&self) -> bool {
        self.fixed
    }

    pub const fn validate(&self) {
        // Runtime composition can replace known edges; opaque parameters alone cannot.
        if !self.fixed || self.count > LIMIT {
            return;
        }
        let mut nodes = [TopologyLeaf::EMPTY; LIMIT];
        self.flatten(&mut nodes, 0);
        if self.closed && self.has_static_scopes {
            validate_scopes(&nodes, self.count);
        }
        let mut color = [0u8; LIMIT];
        let mut stack = [0usize; LIMIT];
        let mut next = [0usize; LIMIT];
        let mut start = 0;
        while start < self.count {
            if color[start] == 0 {
                let mut depth = 1;
                stack[0] = start;
                color[start] = 1;
                while depth != 0 {
                    let node = stack[depth - 1];
                    if next[node] == nodes[node].targets.len() {
                        color[node] = 2;
                        depth -= 1;
                    } else {
                        let target = nodes[node].targets[next[node]];
                        next[node] += 1;
                        let target = match target {
                            Some(target) => target,
                            None => continue,
                        };
                        assert!(target < self.count, "static dependency outside topology");
                        if color[target] == 1 {
                            let diagnostic = describe_cycle(&nodes, &stack, &next, depth, target, self.closed);
                            panic!("{}", diagnostic.message());
                        }
                        if color[target] == 0 {
                            color[target] = 1;
                            stack[depth] = target;
                            depth += 1;
                        }
                    }
                }
            }
            start += 1;
        }
    }
}

const fn validate_scopes(nodes: &[TopologyLeaf; LIMIT], count: usize) {
    let mut consumer = 0;
    while consumer < count {
        if let Some(from) = nodes[consumer].scope {
            let mut parameter = 0;
            while parameter < nodes[consumer].targets.len() {
                if let Some(target) = nodes[consumer].targets[parameter] {
                    assert!(target < count, "static dependency outside topology");
                    if let Some(to) = nodes[target].scope {
                        if !from.can_access(to) {
                            let mut diagnostic = TopologyDiagnostic::new();
                            diagnostic.append("incompatible static scopes: ");
                            diagnostic.append(from.name);
                            diagnostic.append(" -> ");
                            diagnostic.append(to.name);
                            diagnostic.append("\n  ");
                            diagnostic.append(nodes[consumer].source);
                            diagnostic.append("\n  -- parameter #");
                            diagnostic.number(parameter + 1);
                            diagnostic.append(" --> ");
                            diagnostic.append(nodes[target].source);
                            panic!("{}", diagnostic.message());
                        }
                    }
                }
                parameter += 1;
            }
        }
        consumer += 1;
    }
}

const fn describe_cycle(
    nodes: &[TopologyLeaf; LIMIT],
    stack: &[usize; LIMIT],
    next: &[usize; LIMIT],
    depth: usize,
    target: usize,
    closed: bool,
) -> TopologyDiagnostic {
    let mut start = 0;
    while stack[start] != target {
        start += 1;
    }
    let mut diagnostic = TopologyDiagnostic::new();
    if closed {
        diagnostic.append("dependency cycle in closed static registry");
    } else {
        diagnostic.append("dependency cycle in known static dependencies");
    }
    if start + 1 == depth {
        diagnostic.append(" (self-dependency):\n  ");
        if diagnostic.len + nodes[target].source.len() + 128 > diagnostic.bytes.len() {
            diagnostic.append("... (cycle diagnostic truncated)");
        } else {
            diagnostic.append(nodes[target].source);
        }
        diagnostic.append("\n  parameter #");
        diagnostic.number(next[target]);
        diagnostic.append(" requests this registration's own provided value\n");
        return diagnostic;
    }
    diagnostic.append(":\n");
    let mut position = start;
    while position <= depth {
        let node = if position == depth { target } else { stack[position] };
        if position == start {
            diagnostic.append("  ");
        } else {
            diagnostic.append("  -- parameter #");
            diagnostic.number(next[stack[position - 1]]);
            diagnostic.append(" --> ");
        }
        if diagnostic.len + nodes[node].source.len() + 128 > diagnostic.bytes.len() {
            diagnostic.append("... (cycle diagnostic truncated)");
            break;
        }
        diagnostic.append(nodes[node].source);
        diagnostic.append("\n");
        position += 1;
    }
    diagnostic
}

// Bound const-evaluation work and compiler output, including long names and source paths.
struct TopologyDiagnostic {
    bytes: [u8; 4096],
    len: usize,
}

impl TopologyDiagnostic {
    const fn new() -> Self {
        Self { bytes: [0; 4096], len: 0 }
    }

    const fn message(&self) -> &str {
        match from_utf8(self.bytes.split_at(self.len).0) {
            Ok(message) => message,
            Err(_) => panic!("invalid topology diagnostic"),
        }
    }

    const fn append(&mut self, text: &str) -> bool {
        let bytes = text.as_bytes();
        // Copy complete strings, so a bounded diagnostic remains valid UTF-8.
        if self.len + bytes.len() > self.bytes.len() {
            return false;
        }
        let mut index = 0;
        while index < bytes.len() {
            self.bytes[self.len] = bytes[index];
            self.len += 1;
            index += 1;
        }
        true
    }

    const fn number(&mut self, mut value: usize) {
        let mut digits = [0u8; 20];
        let mut len = 0;
        loop {
            digits[len] = b'0' + (value % 10) as u8;
            len += 1;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        if self.len + len > self.bytes.len() {
            return;
        }
        while len != 0 {
            len -= 1;
            self.bytes[self.len] = digits[len];
            self.len += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{from_utf8, Topology, TopologyDiagnostic};
    use crate::ScopeData;
    use alloc::string::String;
    use std::panic::catch_unwind;

    const CYCLE: Topology = Topology::leaf(&[Some(0)]);
    const N2: Topology = Topology::branch(&CYCLE, &CYCLE);
    const N4: Topology = Topology::branch(&N2, &N2);
    const N8: Topology = Topology::branch(&N4, &N4);
    const N16: Topology = Topology::branch(&N8, &N8);
    const N32: Topology = Topology::branch(&N16, &N16);
    const N64: Topology = Topology::branch(&N32, &N32);
    const N128: Topology = Topology::branch(&N64, &N64);
    const N256: Topology = Topology::branch(&N128, &N128);
    const N512: Topology = Topology::branch(&N256, &N256);
    const AT_LIMIT: Topology = Topology::branch(&N512, &N512);
    const OVER_LIMIT: Topology = Topology::branch(&AT_LIMIT, &CYCLE);
    const FALLBACK: () = OVER_LIMIT.validate();

    #[test]
    #[should_panic(expected = "dependency cycle in closed static registry")]
    fn checks_the_last_supported_size() {
        AT_LIMIT.validate();
    }

    #[test]
    fn larger_graphs_defer_even_cycles_to_runtime() {
        let () = FALLBACK;
        const PARTIAL: () = Topology::branch(&AT_LIMIT, &Topology::leaf(&[None])).validate();
        let () = PARTIAL;
    }

    #[test]
    fn opaque_parameters_do_not_disable_known_cycle_checks() {
        const GRAPH: Topology = Topology::leaf(&[None, Some(0)]).with_source("provide(inst)");

        assert!(!GRAPH.can_validate_all_cycles());
        let error = catch_unwind(|| GRAPH.validate()).unwrap_err();
        assert_eq!(
            error.downcast_ref::<String>().unwrap(),
            "dependency cycle in known static dependencies (self-dependency):\n  provide(inst)\n  parameter #2 requests this registration's own provided value\n"
        );
    }

    #[test]
    fn opaque_dags_still_require_runtime_validation() {
        const GRAPH: Topology = Topology::branch(&Topology::leaf(&[None, Some(1)]), &Topology::leaf(&[]));
        const CHECKED: () = GRAPH.validate();

        let () = CHECKED;
        assert!(!GRAPH.can_validate_all_cycles());
    }

    #[test]
    #[should_panic(expected = "dependency cycle in known static dependencies")]
    fn unrelated_opaque_parameters_do_not_hide_cycles() {
        const GRAPH: Topology = Topology::branch(&Topology::leaf(&[None]), &Topology::leaf(&[Some(1)]));

        GRAPH.validate();
    }

    #[test]
    fn changeable_compositions_defer_known_cycles() {
        const GRAPH: Topology = Topology::branch(&Topology::leaf(&[None, Some(0)]), &Topology::OPEN);
        const CHECKED: () = GRAPH.validate();

        let () = CHECKED;
        assert!(!GRAPH.can_validate_all_cycles());
    }

    #[test]
    fn reports_only_the_cycle_in_dependency_order() {
        const PREFIX: Topology = Topology::leaf(&[Some(1)]).with_source("unrelated instantiator");
        const FIRST: Topology = Topology::leaf(&[Some(2)]).with_source("provide(préparer) at src/app.rs:12:5");
        const SECOND: Topology = Topology::leaf(&[Some(1)]).with_source("provide(handler) at src/app.rs:20:5");
        const GRAPH: Topology = Topology::branch(&PREFIX, &Topology::branch(&FIRST, &SECOND));

        let error = catch_unwind(|| GRAPH.validate()).unwrap_err();
        assert_eq!(
            error.downcast_ref::<String>().unwrap(),
            "dependency cycle in closed static registry:\n  provide(préparer) at src/app.rs:12:5\n  -- parameter #1 --> provide(handler) at src/app.rs:20:5\n  -- parameter #1 --> provide(préparer) at src/app.rs:12:5\n"
        );
    }

    #[test]
    fn long_diagnostics_are_explicitly_truncated() {
        const BYTES: [u8; 4096] = [b'x'; 4096];
        const SOURCE: &str = match core::str::from_utf8(&BYTES) {
            Ok(source) => source,
            Err(_) => panic!("invalid test source"),
        };
        const GRAPH: Topology = Topology::leaf(&[Some(0)]).with_source(SOURCE);

        let error = catch_unwind(|| GRAPH.validate()).unwrap_err();
        assert_eq!(
            error.downcast_ref::<String>().unwrap(),
            "dependency cycle in closed static registry (self-dependency):\n  ... (cycle diagnostic truncated)\n  parameter #1 requests this registration's own provided value\n"
        );
    }

    #[test]
    fn identifies_the_parameter_that_requests_its_own_output() {
        const INST: Topology = Topology::leaf(&[Some(1), Some(0)]).with_source("provide(inst) at src/app.rs:12:5");
        const BASE: Topology = Topology::leaf(&[]);
        const GRAPH: Topology = Topology::branch(&INST, &BASE);

        let error = catch_unwind(|| GRAPH.validate()).unwrap_err();
        assert_eq!(
            error.downcast_ref::<String>().unwrap(),
            "dependency cycle in closed static registry (self-dependency):\n  provide(inst) at src/app.rs:12:5\n  parameter #2 requests this registration's own provided value\n"
        );
    }

    const APP: ScopeData = ScopeData {
        priority: 1,
        name: "app",
        is_skipped_by_default: false,
    };
    const REQUEST: ScopeData = ScopeData {
        priority: 3,
        name: "request",
        is_skipped_by_default: false,
    };
    const STATIC_DEPENDENCY: Topology = Topology::leaf(&[]).with_source("provide(repository)").with_scope(Some(&REQUEST));
    const STATIC_CONSUMER: Topology = Topology::leaf(&[Some(1)]).with_source("provide(service)").with_scope(Some(&APP));
    const INVALID_SCOPES: Topology = Topology::branch(&STATIC_CONSUMER, &STATIC_DEPENDENCY);

    #[test]
    fn scope_diagnostic_identifies_both_registrations_and_scopes() {
        let error = catch_unwind(|| INVALID_SCOPES.validate()).unwrap_err();
        assert_eq!(
            error.downcast_ref::<String>().unwrap(),
            "incompatible static scopes: app -> request\n  provide(service)\n  -- parameter #1 --> provide(repository)"
        );
    }

    #[test]
    fn long_scope_diagnostic_preserves_the_validation_error() {
        const PREFIX: &str = "incompatible static scopes: app -> request\n  ";
        const PARAMETER: &str = "\n  -- parameter #";
        const SOURCE_LEN: usize = TopologyDiagnostic::new().bytes.len() - PREFIX.len() - PARAMETER.len();
        const LONG_SOURCE: &str = match from_utf8(&[b'x'; SOURCE_LEN]) {
            Ok(source) => source,
            Err(_) => panic!("invalid test source"),
        };
        const CONSUMER: Topology = Topology::leaf(&[Some(1)]).with_source(LONG_SOURCE).with_scope(Some(&APP));
        let error = catch_unwind(|| Topology::branch(&CONSUMER, &STATIC_DEPENDENCY).validate()).unwrap_err();
        let message = error.downcast_ref::<String>().unwrap();
        assert!(message.starts_with(PREFIX), "{message}");
    }

    #[test]
    fn scopes_with_dynamic_or_changeable_selection_defer_to_runtime() {
        const DYNAMIC_ENDPOINT: () = Topology::branch(&STATIC_CONSUMER, &Topology::leaf(&[])).validate();
        const OPEN: () = Topology::branch(&INVALID_SCOPES, &Topology::OPEN).validate();
        const OPAQUE: () = Topology::branch(&INVALID_SCOPES, &Topology::leaf(&[None])).validate();
        const LARGE: () = Topology::branch(&OVER_LIMIT, &INVALID_SCOPES).validate();
        let () = DYNAMIC_ENDPOINT;
        let () = OPEN;
        let () = OPAQUE;
        let () = LARGE;
    }
}
