//! `Graph -> CompiledGraph`: the structural work over the whole registry, done once.
//!
//! Rules taken from current Froodi (`Registry::validate` in `froodi/src/registry.rs`):
//! a dependency cycle is an error whatever the request mode, and a registration must not depend on
//! a registration of a narrower scope. On top of them the compiler reports what Froodi finds only
//! when a value is resolved, or never: missing bindings, duplicate bindings, scopes outside the
//! hierarchy. The output depends only on the input order.

use alloc::{collections::BTreeSet, vec, vec::Vec};

use crate::diagnostic::{Diagnostic, Diagnostics, PathStep};
use crate::ir::{ExecutionKind, Graph, Registration, RegistrationId, RequestMode, ScopeKey, Target, ValueSource};

/// Position of a scope in the sorted hierarchy: 0 is the widest (lowest priority) scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeId(pub u8);

impl ScopeId {
    #[inline]
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompiledEdge {
    pub target: RegistrationId,
    pub mode: RequestMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledNode {
    pub id: RegistrationId,
    pub type_name: &'static str,
    /// Owning scope.
    pub scope: ScopeId,
    /// `Config::cache_provides`, independent of the scope.
    pub cache_provides: bool,
    pub finalizer: Option<ExecutionKind>,
    pub execution: ExecutionKind,
    pub source: ValueSource,
    /// Dependency edges in parameter order.
    pub edges: Vec<CompiledEdge>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledGraph<K> {
    nodes: Vec<CompiledNode>,
    scopes: Vec<ScopeKey>,
    order: Vec<RegistrationId>,
    keys: Vec<(K, RegistrationId)>,
    /// Transitive dependencies of each node as a bit set over registration ids.
    reach: Vec<Vec<u64>>,
}

impl<K: Ord> CompiledGraph<K> {
    #[must_use]
    pub fn lookup(&self, key: &K) -> Option<RegistrationId> {
        self.keys
            .binary_search_by(|(probe, _)| probe.cmp(key))
            .ok()
            .map(|index| self.keys[index].1)
    }

    /// Construction order: every registration appears after all of its dependencies.
    #[must_use]
    pub fn order(&self) -> &[RegistrationId] {
        &self.order
    }

    /// The scope hierarchy sorted from widest to narrowest.
    #[must_use]
    pub fn scopes(&self) -> &[ScopeKey] {
        &self.scopes
    }

    /// Whether `to` is a direct or transitive dependency of `from`.
    #[must_use]
    pub fn reaches(&self, from: RegistrationId, to: RegistrationId) -> bool {
        self.reach[from.index()][to.index() / 64] & (1 << (to.index() % 64)) != 0
    }

    /// Direct and transitive dependencies of `from`, in id order.
    pub fn reachable(&self, from: RegistrationId) -> impl Iterator<Item = RegistrationId> + '_ {
        (0..self.nodes.len()).map(id).filter(move |&to| self.reaches(from, to))
    }

    /// Every node, indexed by registration id.
    #[must_use]
    pub fn nodes(&self) -> &[CompiledNode] {
        &self.nodes
    }

    #[must_use]
    pub fn node(&self, id: RegistrationId) -> &CompiledNode {
        &self.nodes[id.index()]
    }
}

/// Compiles a registration graph.
///
/// # Errors
/// Returns every diagnostic found, in a stable order: scopes outside the hierarchy, duplicate
/// bindings, missing bindings, scope violations, cycles.
///
/// # Panics
/// Panics if a frontend passes a [`Target::Id`] outside the graph, or more than 255 scopes.
pub fn compile<K: Ord + Clone>(graph: Graph<K>) -> Result<CompiledGraph<K>, Diagnostics> {
    let Graph { registrations, mut scopes } = graph;
    scopes.sort_by_key(|scope| scope.priority);
    scopes.dedup();
    assert!(u8::try_from(scopes.len()).is_ok(), "too many scopes");

    let mut diagnostics = Vec::new();
    let node_scopes = assign_scopes(&registrations, &scopes, &mut diagnostics);
    let keys = index_keys(&registrations, &mut diagnostics);
    let (edges, missing) = resolve_edges(&registrations, &keys);

    let step = |index: usize| PathStep {
        type_name: registrations[index].type_name,
        origin: registrations[index].origin,
    };
    let dependents = dependents(&edges);
    for (index, request_index) in missing {
        let mut path: Vec<PathStep> = path_from_root(&dependents, index).into_iter().map(step).collect();
        path.push(step(index));
        diagnostics.push(Diagnostic::MissingBinding {
            missing: registrations[index].requests[request_index].type_name,
            path,
        });
    }
    check_scope_access(&registrations, &edges, &mut diagnostics);
    for cycle in find_cycles(&edges) {
        diagnostics.push(Diagnostic::Cycle {
            path: cycle.into_iter().map(step).collect(),
        });
    }
    if !diagnostics.is_empty() {
        return Err(Diagnostics(diagnostics));
    }

    let order = topological_order(&edges);
    let reach = reachability(&edges, &order);
    let nodes = registrations
        .iter()
        .zip(edges)
        .zip(node_scopes)
        .enumerate()
        .map(|(index, ((registration, edges), scope))| CompiledNode {
            id: id(index),
            type_name: registration.type_name,
            scope,
            cache_provides: registration.cache_provides,
            finalizer: registration.finalizer,
            execution: registration.execution,
            source: registration.source,
            edges,
        })
        .collect();
    Ok(CompiledGraph {
        nodes,
        scopes,
        order,
        keys,
        reach,
    })
}

fn assign_scopes<K>(registrations: &[Registration<K>], scopes: &[ScopeKey], diagnostics: &mut Vec<Diagnostic>) -> Vec<ScopeId> {
    registrations
        .iter()
        .map(|registration| {
            if let Some(position) = scopes.iter().position(|scope| *scope == registration.scope) {
                ScopeId(u8::try_from(position).expect("scope count checked"))
            } else {
                diagnostics.push(Diagnostic::UnknownScope {
                    registration: PathStep {
                        type_name: registration.type_name,
                        origin: registration.origin,
                    },
                    scope: registration.scope,
                });
                ScopeId(0)
            }
        })
        .collect()
}

/// Sorted `(key, id)` pairs with one entry per key. Every run of equal keys is reported as a
/// duplicate, in order of its first registration.
fn index_keys<K: Ord + Clone>(registrations: &[Registration<K>], diagnostics: &mut Vec<Diagnostic>) -> Vec<(K, RegistrationId)> {
    let mut keys: Vec<(K, RegistrationId)> = registrations
        .iter()
        .enumerate()
        .map(|(index, registration)| (registration.key.clone(), id(index)))
        .collect();
    keys.sort_by(|(a, a_id), (b, b_id)| a.cmp(b).then(a_id.cmp(b_id)));

    let mut duplicates: Vec<Vec<RegistrationId>> = Vec::new();
    let mut start = 0;
    while start < keys.len() {
        let end = start + keys[start..].iter().take_while(|(key, _)| *key == keys[start].0).count();
        if end - start > 1 {
            duplicates.push(keys[start..end].iter().map(|(_, id)| *id).collect());
        }
        start = end;
    }
    duplicates.sort();
    for group in duplicates {
        diagnostics.push(Diagnostic::Duplicate {
            type_name: registrations[group[0].index()].type_name,
            origins: group.iter().map(|id| registrations[id.index()].origin).collect(),
        });
    }
    keys.dedup_by(|b, a| a.0 == b.0);
    keys
}

/// Resolved edges per registration, and the `(registration, request)` pairs nothing provides.
fn resolve_edges<K: Ord>(registrations: &[Registration<K>], keys: &[(K, RegistrationId)]) -> (Vec<Vec<CompiledEdge>>, Vec<(usize, usize)>) {
    let lookup = |key: &K| keys.binary_search_by(|(probe, _)| probe.cmp(key)).ok().map(|index| keys[index].1);
    let mut missing = Vec::new();
    let mut edges = Vec::with_capacity(registrations.len());
    for (index, registration) in registrations.iter().enumerate() {
        let mut node_edges = Vec::with_capacity(registration.requests.len());
        for (request_index, request) in registration.requests.iter().enumerate() {
            if request.mode == RequestMode::Resolver {
                continue;
            }
            let target = match &request.target {
                Target::Id(id) => {
                    assert!(id.index() < registrations.len(), "request targets a registration outside the graph");
                    Some(*id)
                }
                Target::Key(key) => lookup(key),
            };
            match target {
                Some(target) => node_edges.push(CompiledEdge {
                    target,
                    mode: request.mode,
                }),
                None => missing.push((index, request_index)),
            }
        }
        edges.push(node_edges);
    }
    (edges, missing)
}

/// Froodi's rule: a dependency must live in an equal or wider scope.
fn check_scope_access<K>(registrations: &[Registration<K>], edges: &[Vec<CompiledEdge>], diagnostics: &mut Vec<Diagnostic>) {
    for (index, node_edges) in edges.iter().enumerate() {
        for edge in node_edges {
            let dependency = edge.target.index();
            let dependent_scope = registrations[index].scope;
            let dependency_scope = registrations[dependency].scope;
            if dependency_scope.priority > dependent_scope.priority {
                diagnostics.push(Diagnostic::ScopeViolation {
                    dependent: PathStep {
                        type_name: registrations[index].type_name,
                        origin: registrations[index].origin,
                    },
                    dependent_scope,
                    dependency: PathStep {
                        type_name: registrations[dependency].type_name,
                        origin: registrations[dependency].origin,
                    },
                    dependency_scope,
                });
            }
        }
    }
}

#[inline]
fn id(index: usize) -> RegistrationId {
    RegistrationId(u32::try_from(index).expect("too many registrations"))
}

fn dependents(edges: &[Vec<CompiledEdge>]) -> Vec<Vec<usize>> {
    let mut dependents = vec![Vec::new(); edges.len()];
    for (index, node_edges) in edges.iter().enumerate() {
        for edge in node_edges {
            let list: &mut Vec<usize> = &mut dependents[edge.target.index()];
            if !list.contains(&index) {
                list.push(index);
            }
        }
    }
    dependents
}

/// Kahn's algorithm over "dependency before dependent", lowest id first.
fn topological_order(edges: &[Vec<CompiledEdge>]) -> Vec<RegistrationId> {
    let dependents = dependents(edges);
    let mut pending: Vec<usize> = edges
        .iter()
        .map(|node_edges| node_edges.iter().map(|edge| edge.target).collect::<BTreeSet<_>>().len())
        .collect();
    let mut ready: BTreeSet<usize> = (0..edges.len()).filter(|&index| pending[index] == 0).collect();
    let mut order = Vec::with_capacity(edges.len());
    while let Some(index) = ready.pop_first() {
        order.push(id(index));
        for &dependent in &dependents[index] {
            pending[dependent] -= 1;
            if pending[dependent] == 0 {
                ready.insert(dependent);
            }
        }
    }
    order
}

/// Registrations leading to `index`, from a root (nothing depends on it) down to the direct
/// dependent of `index`. Picks the lowest id at each step so the path is deterministic.
fn path_from_root(dependents: &[Vec<usize>], index: usize) -> Vec<usize> {
    let mut path = Vec::new();
    let mut seen = vec![false; dependents.len()];
    seen[index] = true;
    let mut current = index;
    while let Some(&parent) = dependents[current].iter().filter(|&&parent| !seen[parent]).min() {
        seen[parent] = true;
        path.push(parent);
        current = parent;
    }
    path.reverse();
    path
}

/// One cycle per loop, each as a path whose first node is repeated at the end. A node already
/// reported in a cycle does not start another report.
fn find_cycles(edges: &[Vec<CompiledEdge>]) -> Vec<Vec<usize>> {
    const WHITE: u8 = 0;
    const GREY: u8 = 1;
    const BLACK: u8 = 2;

    let mut color = vec![WHITE; edges.len()];
    let mut reported = vec![false; edges.len()];
    let mut cycles = Vec::new();

    for start in 0..edges.len() {
        if color[start] != WHITE {
            continue;
        }
        // Iterative DFS: (node, next edge index).
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        color[start] = GREY;
        while let Some((node, next)) = stack.last_mut().map(|(node, next)| (*node, next)) {
            let Some(edge) = edges[node].get(*next) else {
                color[node] = BLACK;
                stack.pop();
                continue;
            };
            *next += 1;
            let target = edge.target.index();
            match color[target] {
                WHITE => {
                    color[target] = GREY;
                    stack.push((target, 0));
                }
                GREY if !reported[target] => {
                    let from = stack.iter().position(|&(n, _)| n == target).expect("grey node is on the stack");
                    let mut cycle: Vec<usize> = stack[from..].iter().map(|&(n, _)| n).collect();
                    for &n in &cycle {
                        reported[n] = true;
                    }
                    cycle.push(target);
                    cycles.push(cycle);
                }
                _ => {}
            }
        }
    }
    cycles
}

/// Bit set of transitive dependencies per node, filled in construction order so every
/// dependency's set is complete before it is merged.
fn reachability(edges: &[Vec<CompiledEdge>], order: &[RegistrationId]) -> Vec<Vec<u64>> {
    let words = edges.len().div_ceil(64);
    let mut reach = vec![vec![0u64; words]; edges.len()];
    for node in order {
        let mut set = vec![0u64; words];
        for edge in &edges[node.index()] {
            let target = edge.target.index();
            set[target / 64] |= 1 << (target % 64);
            for (word, bits) in set.iter_mut().zip(&reach[target]) {
                *word |= bits;
            }
        }
        reach[node.index()] = set;
    }
    reach
}
